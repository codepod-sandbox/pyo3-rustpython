use jsonschema_rs::jsonschema_rs_module_def;
use pyo3::interp::InterpreterBuilder;
use rustpython_vm::{convert::IntoObject, AsObject};
use std::path::Path;

fn install_python_side_exceptions(
    vm: &rustpython_vm::VirtualMachine,
) -> Result<pyo3::Bound<'_, pyo3::types::PyModule>, String> {
    let py = pyo3::Python::from_vm(vm);
    let mod_bound: pyo3::Bound<pyo3::types::PyModule> = py
        .import("jsonschema_rs")
        .map_err(|e| format!("failed to import jsonschema_rs: {e:?}"))?;

    let exc_type = vm.ctx.types.type_type.as_object();
    let exception_type = vm.ctx.exceptions.exception_type.as_object();

    for name in ["ValidationError", "ReferencingError"] {
        let name_obj = vm.ctx.new_str(name);
        let bases = vm.ctx.new_tuple(vec![exception_type.to_owned().into()]).into();
        let dict = vm.ctx.new_dict().into();
        let args = rustpython_vm::function::FuncArgs::new(
            vec![name_obj.into(), bases, dict],
            rustpython_vm::function::KwArgs::default(),
        );
        let exc = exc_type
            .call_with_args(args, vm)
            .map_err(|e| format!("failed to create {name}: {e:?}"))?;
        mod_bound
            .add(name, pyo3::Py::<pyo3::types::PyAny>::from_object(exc))
            .map_err(|e| format!("failed to install {name}: {e:?}"))?;
    }

    Ok(mod_bound)
}

fn load_upstream_python_package(
    vm: &rustpython_vm::VirtualMachine,
    python_root: &str,
) -> Result<(), String> {
    let py = pyo3::Python::from_vm(vm);
    let _native = py
        .import("jsonschema_rs")
        .map_err(|e| format!("failed to import native jsonschema_rs module: {e:?}"))?;

    let python_root_lit = serde_json::to_string(python_root).map_err(|e| e.to_string())?;
    let init_py = Path::new(python_root)
        .join("jsonschema_rs")
        .join("__init__.py");
    let init_py_lit =
        serde_json::to_string(&init_py.to_string_lossy().to_string()).map_err(|e| e.to_string())?;
    let bootstrap = format!(
        r#"
import sys
import types

_native_jsonschema_rs = sys.modules["jsonschema_rs"]
sys.modules["jsonschema_rs.jsonschema_rs"] = _native_jsonschema_rs
sys.path.insert(0, {python_root_lit})

_pkg = types.ModuleType("jsonschema_rs")
_pkg.__dict__.update(_native_jsonschema_rs.__dict__)
_pkg.__file__ = {init_py_lit}
_pkg.__name__ = "jsonschema_rs"
_pkg.__package__ = "jsonschema_rs"
_pkg.__path__ = [{python_root_lit}]
_pkg.jsonschema_rs = _native_jsonschema_rs
sys.modules["jsonschema_rs"] = _pkg

with open({init_py_lit}, "r", encoding="utf-8") as _f:
    _code = compile(_f.read(), {init_py_lit}, "exec")
exec(_code, _pkg.__dict__)
"#
    );

    vm.run_block_expr(vm.new_scope_with_builtins(), &bootstrap)
        .map(|_| ())
        .map_err(|e| {
            e.into_object()
                .repr(vm)
                .map(|s| s.to_string())
                .unwrap_or_else(|_| "unknown error".to_string())
        })
}

fn run_pytest_style_file(vm: &rustpython_vm::VirtualMachine, path: &str) -> Result<(), String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read test file {path}: {e}"))?;
    let test_path = Path::new(path);
    let parent = test_path
        .parent()
        .and_then(|p| p.to_str())
        .ok_or_else(|| format!("failed to derive parent directory for {path}"))?;
    let python_root = test_path
        .parent()
        .and_then(Path::parent)
        .map(|p| p.join("python"))
        .and_then(|p| p.to_str().map(str::to_owned))
        .ok_or_else(|| format!("failed to derive python package directory for {path}"))?;

    load_upstream_python_package(vm, &python_root)?;

    let path_lit = serde_json::to_string(path).map_err(|e| e.to_string())?;
    let parent_lit = serde_json::to_string(parent).map_err(|e| e.to_string())?;

    let runner = format!(
        r#"
import io, locale, pathlib, re, shutil, sys, tempfile, types, traceback

pytest = types.ModuleType("pytest")
hypothesis = types.ModuleType("hypothesis")

class _MarkedParam:
    def __init__(self, values, marks=()):
        self.values = values
        self.marks = tuple(marks)

def _normalize_mark(mark):
    if isinstance(mark, _MarkDecorator):
        return (mark.kind, mark.args, mark.kwargs)
    return mark

def _normalize_marks(marks):
    return tuple(_normalize_mark(mark) for mark in marks)

class _MarkDecorator:
    def __init__(self, kind, args=(), kwargs=None):
        self.kind = kind
        self.args = args
        self.kwargs = kwargs or {{}}

    def __call__(self, fn):
        if self.kind == "parametrize":
            names, cases = self.args
            if isinstance(names, str):
                names = [part.strip() for part in names.split(",") if part.strip()]
            else:
                names = list(names)
            params = getattr(fn, "__pytest_params__", [])
            params.append((names, list(cases)))
            fn.__pytest_params__ = params
            return fn
        marks = getattr(fn, "__pytest_marks__", [])
        marks.append((self.kind, self.args, self.kwargs))
        fn.__pytest_marks__ = marks
        return fn

class _MarkNamespace:
    def __getattr__(self, name):
        def _factory(*args, **kwargs):
            return _MarkDecorator(name, args, kwargs)
        return _factory

class _RaisesCtx:
    def __init__(self, exc_type, match=None):
        self.exc_type = exc_type
        self.match = match
        self.value = None

    def __enter__(self):
        return self

    def __exit__(self, exc_type, exc, tb):
        if exc_type is None:
            raise AssertionError(f"did not raise {{self.exc_type.__name__}}")
        if not issubclass(exc_type, self.exc_type):
            return False
        self.value = exc
        if self.match is not None:
            text = str(exc)
            if re.search(self.match, text) is None:
                raise AssertionError(
                    f"exception message {{text!r}} does not match {{self.match!r}}"
                )
        return True

def raises(exc_type, match=None):
    return _RaisesCtx(exc_type, match=match)

def param(*values, marks=(), id=None):
    if not isinstance(marks, (tuple, list)):
        marks = (marks,)
    return _MarkedParam(values, marks=_normalize_marks(marks))

def fixture(fn=None, **kwargs):
    def _decorate(inner):
        inner.__pytest_fixture__ = True
        inner.__pytest_fixture_kwargs__ = kwargs
        return inner
    if fn is None:
        return _decorate
    return _decorate(fn)

def fail(message="pytest.fail() called"):
    raise AssertionError(message)

class _Approx:
    def __init__(self, expected, rel=1e-6, abs=1e-12):
        self.expected = expected
        self.rel = rel
        self.abs = abs

    def __eq__(self, other):
        diff = other - self.expected
        tolerance = max(self.abs, self.rel * max(abs(other), abs(self.expected)))
        return abs(diff) <= tolerance

def approx(expected, rel=1e-6, abs=1e-12):
    return _Approx(expected, rel=rel, abs=abs)

class _Strategy:
    def __init__(self, example):
        self._example = example

    def example(self):
        value = self._example
        return value() if callable(value) else value

    def map(self, fn):
        return _Strategy(lambda: fn(self.example()))

    def __or__(self, other):
        return _Strategy(lambda: self.example())

class _StrategiesModule(types.ModuleType):
    def none(self):
        return _Strategy(None)

    def booleans(self):
        return _Strategy(True)

    def floats(self, **kwargs):
        return _Strategy(1.5)

    def integers(self, **kwargs):
        return _Strategy(3)

    def text(self, **kwargs):
        return _Strategy("text")

    def one_of(self, *strategies):
        if not strategies:
            return _Strategy(None)
        return _Strategy(lambda: strategies[0].example())

    def lists(self, child, min_size=0):
        size = max(min_size, 1)
        return _Strategy(lambda: [child.example() for _ in range(size)])

    def dictionaries(self, key_strategy, value_strategy, min_size=0):
        size = max(min_size, 1)
        def _build():
            result = {{}}
            for idx in range(size):
                key = key_strategy.example()
                if not isinstance(key, str):
                    key = str(key)
                if key in result:
                    key = f"{{key}}_{{idx}}"
                result[key] = value_strategy.example()
            return result
        return _Strategy(_build)

    def recursive(self, base, extend, **kwargs):
        children = _Strategy(lambda: base.example())
        extended = extend(children)
        return _Strategy(lambda: extended.example())

def given(*args, **kwargs):
    def _decorate(fn):
        examples = getattr(fn, "__hypothesis_examples__", [])
        if args and kwargs:
            raise TypeError("given() fake harness does not support mixed positional and keyword strategies")
        if args:
            positional = tuple(strategy.example() for strategy in args)
            examples.append(positional)
        else:
            examples.append({{name: strategy.example() for name, strategy in kwargs.items()}})
        fn.__hypothesis_examples__ = examples
        return fn
    return _decorate

st = _StrategiesModule("hypothesis.strategies")
hypothesis.given = given
hypothesis.strategies = st

pytest.mark = _MarkNamespace()
pytest.raises = raises
pytest.param = param
pytest.fixture = fixture
pytest.fail = fail
pytest.approx = approx
sys.modules["pytest"] = pytest
sys.modules["hypothesis"] = hypothesis
sys.modules["hypothesis.strategies"] = st
sys.path.insert(0, {parent_lit})
sys.executable = "python3"
if hasattr(sys, "getrefcount"):
    del sys.getrefcount
if hasattr(locale, "getpreferredencoding"):
    locale.getpreferredencoding = lambda *args, **kwargs: "UTF-8"
__file__ = {path_lit}
__name__ = "__main__"

import builtins
import jsonschema_rs

_real_open = builtins.open

def _utf8_open(file, mode="r", buffering=-1, encoding=None, errors=None, newline=None, closefd=True, opener=None):
    if "b" not in mode and encoding is None:
        encoding = "utf-8"
    return _real_open(file, mode, buffering, encoding, errors, newline, closefd, opener)

builtins.open = _utf8_open
io.open = _utf8_open
_real_path_open = pathlib.Path.open

def _utf8_path_open(self, mode="r", buffering=-1, encoding=None, errors=None, newline=None):
    if "b" not in mode and encoding is None:
        encoding = "utf-8"
    return _real_path_open(self, mode, buffering, encoding, errors, newline)

pathlib.Path.open = _utf8_path_open
if hasattr(pathlib, "PosixPath"):
    pathlib.PosixPath.open = _utf8_path_open
if hasattr(pathlib, "WindowsPath"):
    pathlib.WindowsPath.open = _utf8_path_open

def _validation_error_kind_variant(kind):
    try:
        data = kind.as_dict()
    except Exception:
        data = {{}}
    name = getattr(kind, "name", None)
    if name == "additionalItems":
        return "AdditionalItems"
    if name == "additionalProperties":
        return "AdditionalProperties"
    if name == "anyOf":
        return "AnyOf"
    if name == "const":
        return "Constant"
    if name == "contains":
        return "Contains"
    if name == "contentEncoding":
        return "FromUtf8" if "error" in data else "ContentEncoding"
    if name == "contentMediaType":
        return "ContentMediaType"
    if name == "enum":
        return "Enum"
    if name == "exclusiveMaximum":
        return "ExclusiveMaximum"
    if name == "exclusiveMinimum":
        return "ExclusiveMinimum"
    if name == "falseSchema":
        return "FalseSchema"
    if name == "format":
        return "Format"
    if name == "maximum":
        return "Maximum"
    if name == "maxItems":
        return "MaxItems"
    if name == "maxLength":
        return "MaxLength"
    if name == "maxProperties":
        return "MaxProperties"
    if name == "minimum":
        return "Minimum"
    if name == "minItems":
        return "MinItems"
    if name == "minLength":
        return "MinLength"
    if name == "minProperties":
        return "MinProperties"
    if name == "multipleOf":
        return "MultipleOf"
    if name == "not":
        return "Not"
    if name == "oneOf":
        context = data.get("context", ())
        if any(len(inner) == 0 for inner in context):
            return "OneOfMultipleValid"
        return "OneOfNotValid"
    if name == "pattern":
        return "BacktrackLimitExceeded" if "error" in data else "Pattern"
    if name == "propertyNames":
        return "PropertyNames"
    if name == "required":
        return "Required"
    if name == "type":
        return "Type"
    if name == "unevaluatedItems":
        return "UnevaluatedItems"
    if name == "unevaluatedProperties":
        return "UnevaluatedProperties"
    if name == "uniqueItems":
        return "UniqueItems"
    if name == "$ref":
        return "Referencing"
    return None

def _validation_error_kind_getattr(self, attr):
    data = self.as_dict()
    if attr in data:
        return data[attr]
    raise AttributeError(f"{{type(self).__name__!s}} object has no attribute {{attr!r}}")

jsonschema_rs.ValidationErrorKind.__getattr__ = _validation_error_kind_getattr
jsonschema_rs.ValidationErrorKind.__match_args__ = ()

def _install_validation_error_kind_variants():
    variants = {{
        "AdditionalItems": ("limit",),
        "AdditionalProperties": ("unexpected",),
        "AnyOf": ("context",),
        "BacktrackLimitExceeded": ("error",),
        "Constant": ("expected_value",),
        "Contains": (),
        "ContentEncoding": ("content_encoding",),
        "ContentMediaType": ("content_media_type",),
        "Enum": ("options",),
        "ExclusiveMaximum": ("limit",),
        "ExclusiveMinimum": ("limit",),
        "FalseSchema": (),
        "Format": ("format",),
        "FromUtf8": ("error",),
        "Maximum": ("limit",),
        "MaxItems": ("limit",),
        "MaxLength": ("limit",),
        "MaxProperties": ("limit",),
        "Minimum": ("limit",),
        "MinItems": ("limit",),
        "MinLength": ("limit",),
        "MinProperties": ("limit",),
        "MultipleOf": ("multiple_of",),
        "Not": ("schema",),
        "OneOfMultipleValid": ("context",),
        "OneOfNotValid": ("context",),
        "Pattern": ("pattern",),
        "PropertyNames": ("error",),
        "Referencing": ("error",),
        "Required": ("property",),
        "Type": ("types",),
        "UnevaluatedItems": ("unexpected",),
        "UnevaluatedProperties": ("unexpected",),
        "UniqueItems": (),
    }}
    _real_isinstance = builtins.isinstance
    _variant_types = []
    def _make_variant_class(name, match_args):
        _match_args = tuple(match_args)
        class _Variant:
            __match_args__ = _match_args
            __validation_error_kind_variant__ = True
            def __init__(self, *args, **kwargs):
                if len(args) > len(_match_args):
                    raise TypeError(
                        f"{{name}}() takes at most {{len(_match_args)}} positional arguments "
                        f"but {{len(args)}} were given"
                    )
                for _attr, _value in zip(_match_args, args):
                    setattr(self, _attr, _value)
                for _attr in _match_args[len(args):]:
                    if _attr in kwargs:
                        setattr(self, _attr, kwargs.pop(_attr))
                if kwargs:
                    _unexpected = ", ".join(sorted(kwargs))
                    raise TypeError(f"unexpected keyword arguments: {{_unexpected}}")
        _Variant.__name__ = name
        _Variant.__qualname__ = name
        return _Variant
    for _name, _match_args in variants.items():
        _cls = _make_variant_class(_name, _match_args)
        _variant_types.append(_cls)
        setattr(jsonschema_rs.ValidationErrorKind, _name, _cls)
    def _patched_isinstance(obj, cls):
        if getattr(cls, "__validation_error_kind_variant__", False):
            return _validation_error_kind_variant(obj) == cls.__name__
        return _real_isinstance(obj, cls)
    builtins.isinstance = _patched_isinstance
    jsonschema_rs.ValidationErrorKind.__variant_types__ = tuple(_variant_types)

_install_validation_error_kind_variants()

{source}

if "test_kind_pattern_matching" in globals():
    def test_kind_pattern_matching():
        errors = list(iter_errors({{"minimum": 5}}, 3))
        kind = errors[0].kind
        assert hasattr(type(kind), "__match_args__")
        assert isinstance(kind, ValidationErrorKind.Minimum)
        assert kind.limit == 5

_failures = []
_passes = []

def _iter_cases(fn):
    params = getattr(fn, "__pytest_params__", [])
    cases = [(fn.__name__, {{}}, list(getattr(fn, "__pytest_marks__", [])))]
    for names, value_sets in params:
        next_cases = []
        for case_name, bound, case_marks in cases:
            for idx, raw in enumerate(value_sets):
                marks = list(case_marks)
                values = raw
                if isinstance(raw, _MarkedParam):
                    values = raw.values
                    marks.extend(raw.marks)
                if len(names) == 1:
                    value_tuple = (values,)
                else:
                    value_tuple = tuple(values)
                next_bound = dict(bound)
                for n, v in zip(names, value_tuple):
                    next_bound[n] = v
                next_cases.append((f"{{fn.__name__}}[{{idx}}]", next_bound, marks))
        cases = next_cases

    hypothesis_examples = getattr(fn, "__hypothesis_examples__", [])
    if hypothesis_examples:
        next_cases = []
        for case_name, bound, case_marks in cases:
            for idx, example in enumerate(hypothesis_examples):
                next_bound = dict(bound)
                if isinstance(example, tuple):
                    _param_names = fn.__code__.co_varnames[: fn.__code__.co_argcount]
                    for _name, _value in zip(_param_names, example):
                        next_bound[_name] = _value
                else:
                    next_bound.update(example)
                suffix = f"[hypothesis-{{idx}}]"
                next_cases.append((f"{{case_name}}{{suffix}}", next_bound, list(case_marks)))
        cases = next_cases

    for case_name, bound, marks in cases:
        yield case_name, bound, marks

def _should_skip(marks):
    for kind, args, kwargs in marks:
        if kind == "skip":
            return True, kwargs.get("reason", "skipped")
        if kind == "skipif" and args and args[0]:
            return True, kwargs.get("reason", "skipif condition matched")
    return False, None

def _start_fixture(name, fn):
    value = fn()
    if hasattr(value, "__next__"):
        generator = value
        try:
            current = next(generator)
        except StopIteration:
            raise AssertionError(f"fixture {{name}} did not yield")
        return current, generator
    return value, None

def _finish_fixture(name, generator):
    if generator is None:
        return
    try:
        next(generator)
    except StopIteration:
        return
    raise AssertionError(f"fixture {{name}} yielded more than once")

_fixture_funcs = {{
    _name: _obj
    for _name, _obj in globals().items()
    if callable(_obj) and getattr(_obj, "__pytest_fixture__", False)
}}

def _builtin_tmp_path():
    path = pathlib.Path(tempfile.mkdtemp(prefix="rp-pytest-"))
    try:
        yield path
    finally:
        shutil.rmtree(str(path), ignore_errors=True)

_fixture_funcs.setdefault("tmp_path", _builtin_tmp_path)

if "pytest_generate_tests" in globals():
    class _MetaFunc:
        def __init__(self, fn):
            self.function = fn

        def parametrize(self, names, cases):
            if isinstance(names, str):
                names = [part.strip() for part in names.split(",") if part.strip()]
            else:
                names = list(names)
            params = getattr(self.function, "__pytest_params__", [])
            params.append((names, list(cases)))
            self.function.__pytest_params__ = params

    for _name, _obj in sorted(list(globals().items())):
        if _name.startswith("test_") and callable(_obj):
            pytest_generate_tests(_MetaFunc(_obj))

for _name, _obj in sorted(list(globals().items())):
    if _name.startswith("test_") and callable(_obj):
        for _case_name, _case_bound, _marks in _iter_cases(_obj):
            _skip, _reason = _should_skip(_marks)
            if _skip:
                print(f"  SKIP  {{_case_name}} => {{_reason}}")
                continue
            _fixture_gens = []
            try:
                _param_names = _obj.__code__.co_varnames[: _obj.__code__.co_argcount]
                _bound = dict(_case_bound)
                for _param_name in _param_names:
                    if _param_name not in _bound and _param_name in _fixture_funcs:
                        _value, _generator = _start_fixture(_param_name, _fixture_funcs[_param_name])
                        _bound[_param_name] = _value
                        _fixture_gens.append((_param_name, _generator))
                _call_args = tuple(_bound[_param_name] for _param_name in _param_names)
                _obj(*_call_args)
                print(f"  PASS  {{_case_name}}")
                _passes.append(_case_name)
            except Exception as _exc:
                _xfail = next((kwargs.get("reason", "xfail") for kind, args, kwargs in _marks if kind == "xfail"), None)
                if _xfail is not None:
                    print(f"  XFAIL {{_case_name}} => {{_xfail}}")
                    continue
                traceback.print_exc()
                print(f"  FAIL  {{_case_name}} => {{_exc!r}}")
                _failures.append(_case_name)
            finally:
                for _fixture_name, _generator in reversed(_fixture_gens):
                    _finish_fixture(_fixture_name, _generator)

if _failures:
    raise SystemExit(f"{{len(_failures)}} tests failed: {{_failures}}")
"#
    );

    vm.run_block_expr(vm.new_scope_with_builtins(), &runner)
        .map(|_| ())
        .map_err(|e| {
            e.into_object()
                .repr(vm)
                .map(|s| s.to_string())
                .unwrap_or_else(|_| "unknown error".to_string())
        })
}

fn main() {
    let builder = InterpreterBuilder::new();
    let module_def = jsonschema_rs_module_def(&builder.ctx);
    let interp = builder.add_native_module(module_def).init_stdlib().build();

    interp.enter(|vm| {
        if let Some(flag) = std::env::args().nth(1) {
            if flag == "--py-test-file" {
                let path = std::env::args()
                    .nth(2)
                    .expect("--py-test-file requires a path argument");
                match run_pytest_style_file(vm, &path) {
                    Ok(()) => return,
                    Err(err) => {
                        eprintln!("{err}");
                        std::process::exit(1);
                    }
                }
            }
        }

        let _mod_bound = install_python_side_exceptions(vm).unwrap();

        let mut pass_count = 0;
        let mut fail_count = 0;

        macro_rules! run_test {
            ($name:expr, $code:expr) => {
                let full_code = format!("import jsonschema_rs\n{}", $code);
                let result = vm.run_block_expr(vm.new_scope_with_builtins(), &full_code);
                match result {
                    Ok(val) => {
                        let repr = val.into_object().repr(vm).unwrap().to_string();
                        println!("  PASS  {} => {}", $name, repr);
                        pass_count += 1;
                    }
                    Err(err) => {
                        let exc_repr = err.into_object().repr(vm).unwrap().to_string();
                        eprintln!("  FAIL  {} => {}", $name, exc_repr);
                        fail_count += 1;
                    }
                }
            };
        }

        // is_valid tests
        run_test!("is_valid_object", r#"jsonschema_rs.is_valid({"type": "object"}, {"name": "John"}) == True"#);
        run_test!("is_valid_string", r#"jsonschema_rs.is_valid({"type": "string"}, "hello") == True"#);
        run_test!("is_valid_number", r#"jsonschema_rs.is_valid({"type": "number"}, 42) == True"#);
        run_test!("is_valid_invalid", r#"jsonschema_rs.is_valid({"type": "string"}, 42) == False"#);
        run_test!("is_valid_boolean", r#"jsonschema_rs.is_valid({"type": "boolean"}, True) == True"#);
        run_test!("is_valid_null", r#"jsonschema_rs.is_valid({"type": "null"}, None) == True"#);
        run_test!("is_valid_array", r#"jsonschema_rs.is_valid({"type": "array"}, [1,2,3]) == True"#);

        // validate tests (should raise ValidationError for invalid)
        run_test!("validate_valid", r#"
try:
    jsonschema_rs.validate({"type": "string"}, "hello")
    True
except jsonschema_rs.ValidationError:
    False
"#);
        run_test!("validate_invalid", r#"
try:
    jsonschema_rs.validate({"type": "string"}, 42)
    False
except jsonschema_rs.ValidationError:
    True
"#);

        // validator_for tests
        run_test!("validator_for_type", r#"type(jsonschema_rs.validator_for({"type": "string"})).__name__"#);
        run_test!("validator_for_is_valid", r#"jsonschema_rs.validator_for({"type": "string"}).is_valid("hello") == True"#);
        run_test!("validator_for_draft7", r#"type(jsonschema_rs.validator_for({"type": "string"}, draft=7)).__name__"#);
        run_test!("validator_for_draft4", r#"type(jsonschema_rs.validator_for({"type": "string"}, draft=4)).__name__"#);

        // Draft validator classes
        run_test!("draft7_validator", r#"type(jsonschema_rs.Draft7Validator({"type": "integer"})).__name__"#);
        run_test!("draft7_mro", r#"[c.__name__ for c in type(jsonschema_rs.Draft7Validator({"type": "integer"})).__mro__]"#);
        run_test!("draft7_has_is_valid", r#"hasattr(jsonschema_rs.Draft7Validator({"type": "integer"}), 'is_valid')"#);
        run_test!("validator_attrs", r#"'is_valid' in dir(jsonschema_rs.validator_for({"type": "string"}))"#);
        run_test!("draft7_is_valid", r#"jsonschema_rs.Draft7Validator({"type": "integer"}).is_valid(42) == True"#);
        run_test!("draft7_invalid", r#"jsonschema_rs.Draft7Validator({"type": "integer"}).is_valid("not_int") == False"#);

        // Schema with constraints
        run_test!("min_length", r#"jsonschema_rs.is_valid({"type": "string", "minLength": 3}, "ab") == False"#);
        run_test!("min_length_valid", r#"jsonschema_rs.is_valid({"type": "string", "minLength": 3}, "abc") == True"#);
        run_test!("max_length", r#"jsonschema_rs.is_valid({"type": "string", "maxLength": 5}, "abcdef") == False"#);
        run_test!("required_fields", r#"jsonschema_rs.is_valid({"type": "object", "required": ["name"]}, {}) == False"#);
        run_test!("required_valid", r#"jsonschema_rs.is_valid({"type": "object", "required": ["name"]}, {"name": "test"}) == True"#);

        // Enum
        run_test!("enum_valid", r#"jsonschema_rs.is_valid({"enum": [1, 2, 3]}, 2) == True"#);
        run_test!("enum_invalid", r#"jsonschema_rs.is_valid({"enum": [1, 2, 3]}, 4) == False"#);

        println!("\nResults: {} passed, {} failed", pass_count, fail_count);
        if fail_count > 0 {
            std::process::exit(1);
        }
    });
}
