use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{ItemFn, Result};

pub fn expand(_attr: TokenStream, input: ItemFn) -> Result<TokenStream> {
    let fn_name = &input.sig.ident;
    let fn_name_str = fn_name.to_string();

    // Generated symbol names
    let module_def_fn = format_ident!("{}_module_def", fn_name);
    let exec_fn = format_ident!("__pyo3_{}_exec", fn_name);
    let static_def = format_ident!("__PYO3_{}_DEF", fn_name.to_string().to_uppercase());

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
            #fn_name(&__bound).map_err(|e| ::pyo3::err::into_vm_err(e))
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
                    name: __ctx.intern_str(#fn_name_str),
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
