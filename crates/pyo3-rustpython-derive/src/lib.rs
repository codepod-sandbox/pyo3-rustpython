use proc_macro::TokenStream;

mod pyfunction;
mod pymodule;

/// Marks a function as callable from Python.
///
/// Generates a companion `__pyo3_fn_<name>` function that creates a
/// `Bound<'_, PyAny>` (a `HeapMethodDef`) via `vm.ctx.new_method_def`.
///
/// Supported argument types: `&str`, `String`, `i64`, `i32`, `i16`, `i8`,
/// `u64`, `u32`, `u16`, `u8`, `usize`, `f64`, `f32`, `bool`.
///
/// Use `wrap_pyfunction!(name, module)` to register the result.
#[proc_macro_attribute]
pub fn pyfunction(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    pyfunction::expand(attr.into(), input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Marks a function as a Python module initializer.
///
/// The function must have the signature:
/// ```rust,ignore
/// fn name(m: &Bound<'_, PyModule>) -> PyResult<()> { ... }
/// ```
///
/// Generates a `<name>_module_def(ctx: &Context) -> &'static PyModuleDef`
/// function suitable for use with `config.add_native_module(...)`.
#[proc_macro_attribute]
pub fn pymodule(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    pymodule::expand(attr.into(), input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Stub — `#[pyclass]` support is not yet implemented.
#[proc_macro_attribute]
pub fn pyclass(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Stub — `#[pymethods]` support is not yet implemented.
#[proc_macro_attribute]
pub fn pymethods(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}
