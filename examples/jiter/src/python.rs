//! JSON-to-Python parser adapted for pyo3-rustpython shim.

use std::marker::PhantomData;

use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyList};

use smallvec::SmallVec;

use jiter::errors::{JsonError, JsonResult, DEFAULT_RECURSION_LIMIT};
use jiter::number_decoder::{NumberAny, NumberInt};
use jiter::parse::{Parser, Peek};
use jiter::string_decoder::{StringDecoder, Tape};
use jiter::{json_err, json_error, JsonErrorType, PartialMode};

use crate::py_lossless_float::FloatMode;
use crate::py_string_cache::{StringCacheAll, StringCacheKeys, StringCacheMode, StringMaybeCache, StringNoCache};

#[derive(Default)]
#[allow(clippy::struct_excessive_bools, dead_code)]
pub struct PythonParse {
    pub allow_inf_nan: bool,
    pub cache_mode: StringCacheMode,
    pub partial_mode: PartialMode,
    pub catch_duplicate_keys: bool,
    pub float_mode: FloatMode,
}

impl PythonParse {
    pub fn python_parse<'py>(&self, py: Python<'py>, json_data: &[u8]) -> JsonResult<Bound<'py, PyAny>> {
        macro_rules! ppp {
            ($string_cache:ident, $key_check:ident) => {
                PythonParser::<$string_cache, $key_check>::parse(
                    py,
                    json_data,
                    self.allow_inf_nan,
                    self.partial_mode,
                )
            };
        }
        macro_rules! ppp_group {
            ($string_cache:ident) => {
                match self.catch_duplicate_keys {
                    true => ppp!($string_cache, DuplicateKeyCheck),
                    false => ppp!($string_cache, NoopKeyCheck),
                }
            };
        }

        match self.cache_mode {
            StringCacheMode::All => ppp_group!(StringCacheAll),
            StringCacheMode::Keys => ppp_group!(StringCacheKeys),
            StringCacheMode::None => ppp_group!(StringNoCache),
        }
    }
}

/// Map a `JsonError` to a `PyErr` which can be raised as an exception in Python as a `ValueError`.
pub fn map_json_error(json_data: &[u8], json_error: &JsonError) -> PyErr {
    pyo3::exceptions::PyValueError::new_err(json_error.description(json_data))
}

struct PythonParser<'j, StringCache, KeyCheck> {
    _string_cache: PhantomData<StringCache>,
    _key_check: PhantomData<KeyCheck>,
    parser: Parser<'j>,
    tape: Tape,
    recursion_limit: u8,
    allow_inf_nan: bool,
    partial_mode: PartialMode,
}

impl<StringCache: StringMaybeCache, KeyCheck: MaybeKeyCheck>
    PythonParser<'_, StringCache, KeyCheck>
{
    fn parse<'py>(
        py: Python<'py>,
        json_data: &[u8],
        allow_inf_nan: bool,
        partial_mode: PartialMode,
    ) -> JsonResult<Bound<'py, PyAny>> {
        let mut slf = PythonParser {
            _string_cache: PhantomData::<StringCache>,
            _key_check: PhantomData::<KeyCheck>,
            parser: Parser::new(json_data),
            tape: Tape::default(),
            recursion_limit: DEFAULT_RECURSION_LIMIT,
            allow_inf_nan,
            partial_mode,
        };

        let peek = slf.parser.peek()?;
        let v = slf.py_take_value(py, peek)?;
        if !slf.partial_mode.is_active() {
            slf.parser.finish()?;
        }
        Ok(v)
    }

    fn py_take_value<'py>(&mut self, py: Python<'py>, peek: Peek) -> JsonResult<Bound<'py, PyAny>> {
        match peek {
            Peek::Null => {
                self.parser.consume_null()?;
                Ok(py.None().into_bound(py))
            }
            Peek::True => {
                self.parser.consume_true()?;
                Ok(Bound::<PyBool>::new(py, true).into_any())
            }
            Peek::False => {
                self.parser.consume_false()?;
                Ok(Bound::<PyBool>::new(py, false).into_any())
            }
            Peek::String => {
                let s = self
                    .parser
                    .consume_string::<StringDecoder>(&mut self.tape, self.partial_mode.allow_trailing_str())?;
                Ok(StringCache::get_value(py, s).into_any())
            }
            Peek::Array => {
                let peek_first = match self.parser.array_first() {
                    Ok(Some(peek)) => peek,
                    Err(e) if !self.allow_partial_err(&e) => return Err(e),
                    Ok(None) | Err(_) => return Ok(Bound::<PyList>::empty(py).into_any()),
                };

                let mut vec: SmallVec<[Bound<'_, PyAny>; 8]> = SmallVec::with_capacity(8);
                if let Err(e) = self.parse_array(py, peek_first, &mut vec) {
                    if !self.allow_partial_err(&e) {
                        return Err(e);
                    }
                }

                let list = Bound::<PyList>::new(py, &vec);
                Ok(list.into_any())
            }
            Peek::Object => {
                let dict = Bound::<PyDict>::new(py);
                if let Err(e) = self.parse_object(py, &dict) {
                    if !self.allow_partial_err(&e) {
                        return Err(e);
                    }
                }
                Ok(dict.into_any())
            }
            _ => parse_number(py, &mut self.parser, peek, self.allow_inf_nan),
        }
    }

    fn parse_array<'py>(
        &mut self,
        py: Python<'py>,
        peek_first: Peek,
        vec: &mut SmallVec<[Bound<'py, PyAny>; 8]>,
    ) -> JsonResult<()> {
        let v = self.check_take_value(py, peek_first)?;
        vec.push(v);
        while let Some(peek) = self.parser.array_step()? {
            let v = self.check_take_value(py, peek)?;
            vec.push(v);
        }
        Ok(())
    }

    fn parse_object<'py>(&mut self, py: Python<'py>, dict: &Bound<'py, PyDict>) -> JsonResult<()> {
        let mut check_keys = KeyCheck::default();
        if let Some(first_key) = self.parser.object_first::<StringDecoder>(&mut self.tape)? {
            let first_key_s = first_key.as_str();
            check_keys.check(first_key_s, self.parser.index)?;
            let first_key = StringCache::get_key(py, first_key);
            let peek = self.parser.peek()?;
            let first_value = self.check_take_value(py, peek)?;
            dict.set_item(first_key, first_value)
                .map_err(|e| py_err_to_json_err(&e, self.parser.index))?;
            while let Some(key) = self.parser.object_step::<StringDecoder>(&mut self.tape)? {
                let key_s = key.as_str();
                check_keys.check(key_s, self.parser.index)?;
                let key = StringCache::get_key(py, key);
                let peek = self.parser.peek()?;
                let value = self.check_take_value(py, peek)?;
                dict.set_item(key, value)
                    .map_err(|e| py_err_to_json_err(&e, self.parser.index))?;
            }
        }
        Ok(())
    }

    fn allow_partial_err(&self, e: &JsonError) -> bool {
        if self.partial_mode.is_active() {
            e.allowed_if_partial()
        } else {
            false
        }
    }

    fn check_take_value<'py>(&mut self, py: Python<'py>, peek: Peek) -> JsonResult<Bound<'py, PyAny>> {
        self.recursion_limit = match self.recursion_limit.checked_sub(1) {
            Some(limit) => limit,
            None => return json_err!(RecursionLimitExceeded, self.parser.index),
        };

        let r = self.py_take_value(py, peek);

        self.recursion_limit += 1;
        r
    }
}

/// Parse a number and convert to a Python object.
/// We always use lossy (f64) float mode for simplicity.
fn parse_number<'py>(
    py: Python<'py>,
    parser: &mut Parser,
    peek: Peek,
    allow_inf_nan: bool,
) -> JsonResult<Bound<'py, PyAny>> {
    match parser.consume_number::<NumberAny>(peek.into_inner(), allow_inf_nan) {
        Ok(number) => {
            let obj = number_any_into_pyobject(py, number)
                .map_err(|e| py_err_to_json_err(&e, parser.index))?;
            Ok(obj)
        }
        Err(e) => {
            if !peek.is_num() {
                Err(json_error!(ExpectedSomeValue, parser.index))
            } else {
                Err(e)
            }
        }
    }
}

/// Convert a NumberAny into a Python object.
fn number_any_into_pyobject<'py>(py: Python<'py>, number: NumberAny) -> PyResult<Bound<'py, PyAny>> {
    match number {
        NumberAny::Int(num_int) => number_int_into_pyobject(py, num_int),
        NumberAny::Float(f) => Ok(Bound::<pyo3::types::PyFloat>::new(py, f).into_any()),
    }
}

/// Convert a NumberInt into a Python object.
fn number_int_into_pyobject<'py>(py: Python<'py>, number: NumberInt) -> PyResult<Bound<'py, PyAny>> {
    match number {
        NumberInt::Int(i) => {
            use pyo3::IntoPyObject;
            let obj = i.into_pyobject(py).map_err(|e: PyErr| e)?;
            Ok(obj.into_any())
        }
        NumberInt::BigInt(big_int) => {
            // Convert num_bigint::BigInt to i128 if it fits, otherwise via string
            use std::convert::TryFrom;
            let vm = py.vm();
            if let Ok(val) = i128::try_from(&big_int) {
                let int_obj = vm.ctx.new_int(val);
                let pyobj: rustpython_vm::PyObjectRef = int_obj.into();
                Ok(Bound::from_object(py, pyobj))
            } else {
                // For truly massive ints, call Python's int() on string repr
                let s = big_int.to_string();
                let py_str: rustpython_vm::PyObjectRef = vm.ctx.new_str(s).into();
                let builtins = vm.builtins.clone();
                let int_fn = builtins.get_attr("int", vm)
                    .map_err(|e| PyErr::from_vm_err(e))?;
                let result = int_fn.call((py_str,), vm)
                    .map_err(|e| PyErr::from_vm_err(e))?;
                Ok(Bound::from_object(py, result))
            }
        }
    }
}

use ahash::AHashSet;

trait MaybeKeyCheck: Default {
    fn check(&mut self, key: &str, index: usize) -> JsonResult<()>;
}

#[derive(Default)]
struct NoopKeyCheck;

impl MaybeKeyCheck for NoopKeyCheck {
    fn check(&mut self, _key: &str, _index: usize) -> JsonResult<()> {
        Ok(())
    }
}

#[derive(Default)]
struct DuplicateKeyCheck(AHashSet<String>);

impl MaybeKeyCheck for DuplicateKeyCheck {
    fn check(&mut self, key: &str, index: usize) -> JsonResult<()> {
        if self.0.insert(key.to_owned()) {
            Ok(())
        } else {
            Err(JsonError::new(JsonErrorType::DuplicateKey(key.to_owned()), index))
        }
    }
}

fn py_err_to_json_err(e: &PyErr, index: usize) -> JsonError {
    JsonError::new(JsonErrorType::InternalError(e.to_string()), index)
}
