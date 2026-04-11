use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemFn, Result};

/// Parse the module name from either the attribute args or a `#[pyo3(name = "...")]`
/// attribute on the function.
fn parse_module_name(attr: &TokenStream, input: &ItemFn) -> Option<String> {
    // First check the macro attribute itself: #[pymodule(name = "...", ...)]
    // Options like `gil_used` are parsed and ignored.
    {
        let s = attr.to_string();
        for part in s.split(',') {
            let part = part.trim();
            if let Some(rest) = part.strip_prefix("name") {
                let rest = rest.trim();
                if let Some(rest) = rest.strip_prefix('=') {
                    let rest = rest.trim().trim_matches('"');
                    if !rest.is_empty() {
                        return Some(rest.to_string());
                    }
                }
            }
        }
    }

    // Then check for a sibling #[pyo3(name = "...")] attribute on the function.
    for a in &input.attrs {
        if a.path().is_ident("pyo3") {
            let mut found_name: Option<String> = None;
            let _ = a.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    let value = meta.value()?;
                    let lit: syn::LitStr = value.parse()?;
                    found_name = Some(lit.value());
                }
                Ok(())
            });
            if found_name.is_some() {
                return found_name;
            }
        }
    }
    None
}

pub fn expand(attr: TokenStream, mut input: ItemFn) -> Result<TokenStream> {
    // Strip #[pyo3(...)] attributes from the function so they don't cause
    // "cannot find attribute `pyo3`" errors.
    input.attrs.retain(|a| !a.path().is_ident("pyo3"));
    let fn_name = &input.sig.ident;
    let fn_name_str = fn_name.to_string();

    // Parse and ignore `gil_used = ...` and other options from the attribute.
    // e.g. #[pymodule(gil_used = false)]
    // We only care about `name = "..."` if present.
    let module_name = parse_module_name(&attr, &input).unwrap_or_else(|| fn_name_str.clone());

    // Generated symbol names
    let module_def_fn = format_ident!("{}_module_def", fn_name);
    let exec_fn = format_ident!("__pyo3_{}_exec", fn_name);
    let static_def = format_ident!("__PYO3_{}_DEF", fn_name.to_string().to_uppercase());

    // Detect if the user function takes a `py: Python` parameter before the module.
    // pyo3 supports both:
    //   fn name(m: &Bound<'_, PyModule>) -> PyResult<()>
    //   fn name(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()>
    let param_count = input.sig.inputs.len();
    let call_expr = if param_count >= 2 {
        quote! { #fn_name(__py, &__bound) }
    } else {
        quote! { #fn_name(&__bound) }
    };

    Ok(quote! {
        // Keep the user's init function unchanged.
        #input

        /// Module exec slot: called by RustPython when the module is first imported.
        /// Constructs a `Bound<PyModule>` and delegates to the user's init function.
        #[doc(hidden)]
        fn #exec_fn(
            __vm: &::rustpython_vm::VirtualMachine,
            __module: &::rustpython_vm::Py<::rustpython_vm::builtins::PyModule>,
        ) -> ::rustpython_vm::PyResult<()> {
            let __py = ::pyo3::Python::from_vm(__vm);
            let __bound = ::pyo3::Bound::<::pyo3::types::PyModule>::from_exec_ref(__py, __module);
            #call_expr.map_err(|e| ::pyo3::err::into_vm_err(e))
        }

        /// Returns the `&'static PyModuleDef` for this module.
        ///
        /// Pass the result to `config.add_native_module(...)` when building
        /// the RustPython interpreter.
        pub fn #module_def_fn(
            __ctx: &::rustpython_vm::Context,
        ) -> &'static ::rustpython_vm::builtins::PyModuleDef {
            // `PyModuleDef` is not `Sync` (contains non-atomic interior mutability
            // via `PyStrInterned`), so we store a raw pointer wrapped in our
            // `SyncModuleDefPtr` helper. The WASM / single-VM target is always
            // single-threaded, so the unsafe impls are sound.
            static #static_def: ::std::sync::OnceLock<::pyo3::SyncModuleDefPtr>
                = ::std::sync::OnceLock::new();

            let ptr = #static_def.get_or_init(|| {
                let mut __def = Box::new(::rustpython_vm::builtins::PyModuleDef {
                    name: __ctx.intern_str(#module_name),
                    doc: None,
                    methods: &[],
                    slots: Default::default(),
                });
                __def.slots.exec = Some(#exec_fn);
                ::pyo3::SyncModuleDefPtr(Box::into_raw(__def) as *const _)
            });
            // Safety: pointer was created via `Box::into_raw` and is never freed.
            unsafe { &*ptr.0 }
        }
    })
}
