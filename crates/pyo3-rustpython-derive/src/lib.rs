use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::DeriveInput;

mod pyclass;
mod pyfunction;
mod pymethods;
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

/// Marks a struct or enum as a Python class.
///
/// Generates RustPython's `#[pyclass]` attribute and `#[derive(PyPayload)]` on
/// the type. For structs, fields annotated with `#[pyo3(get)]` and/or
/// `#[pyo3(set)]` get auto-generated getter/setter methods via `#[pygetset]`.
/// For enums, the entire enum is treated as a single Python class.
#[proc_macro_attribute]
pub fn pyclass(attr: TokenStream, item: TokenStream) -> TokenStream {
    let attr2: TokenStream2 = attr.into();
    let input = item.clone();
    if let Ok(s) = syn::parse::<syn::ItemStruct>(input.clone()) {
        pyclass::expand(attr2, s)
            .unwrap_or_else(|e| e.to_compile_error())
            .into()
    } else if let Ok(e) = syn::parse::<syn::ItemEnum>(input) {
        pyclass::expand_enum(attr2, e)
            .unwrap_or_else(|e| e.to_compile_error())
            .into()
    } else {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[pyclass] can only be applied to a struct or enum",
        )
        .to_compile_error()
        .into()
    }
}

/// Marks an impl block as containing Python methods for a `#[pyclass]`.
///
/// Transforms pyo3-style method annotations to RustPython equivalents:
/// - `#[new]` → `Constructor` trait impl
/// - `__repr__`, `__str__`, etc. → `#[pymethod]` (slots wired at registration)
/// - Regular methods → `#[pymethod]`
/// - `#[getter]` / `#[setter]` → `#[pygetset]` / `#[pygetset(setter)]`
/// - `#[staticmethod]` / `#[classmethod]` → `#[pystaticmethod]` / `#[pyclassmethod]`
#[proc_macro_attribute]
pub fn pymethods(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemImpl);
    pymethods::expand(attr.into(), input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// No-op attribute that swallows `#[pyo3(signature = ...)]` and similar
/// annotations inside `#[pymethods]` blocks. PyO3 uses these to configure
/// method signatures; RustPython doesn't need them, so we just pass the
/// item through unchanged.
#[proc_macro_attribute]
pub fn pyo3(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Derives `FromPyObject` for a struct.
///
/// For tuple structs, extracts each field from a Python tuple positionally.
/// For named structs, falls back to a no-op extraction (panics at runtime).
#[proc_macro_derive(FromPyObject, attributes(pyo3))]
pub fn derive_from_py_object(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    derive_from_py_object_impl(input)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn derive_from_py_object_impl(input: DeriveInput) -> syn::Result<TokenStream2> {
    let ident = &input.ident;

    let fields = match &input.data {
        syn::Data::Struct(s) => &s.fields,
        _ => {
            return Err(syn::Error::new_spanned(
                &input.ident,
                "FromPyObject derive only supports structs",
            ));
        }
    };

    match fields {
        syn::Fields::Unnamed(unnamed) => {
            let field_types: Vec<_> = unnamed.unnamed.iter().map(|f| &f.ty).collect();
            let field_pats: Vec<_> = (0..field_types.len())
                .map(|i| quote::format_ident!("_f{}", i))
                .collect();

            let extract_tuple = if field_types.len() == 1 {
                let t = &field_types[0];
                let fp = &field_pats[0];
                quote! {
                    let #fp: #t = ob.extract()?;
                }
            } else {
                quote! {
                    let ( #(#field_pats),* ): ( #(#field_types),* ) = ob.extract()?;
                }
            };

            Ok(quote! {
                impl<'py> ::pyo3::FromPyObject<'py> for #ident {
                    fn extract_bound(ob: &::pyo3::Bound<'py, ::pyo3::types::PyAny>) -> ::pyo3::PyResult<Self> {
                        #extract_tuple
                        Ok(#ident( #(#field_pats),* ))
                    }
                }
            })
        }
        syn::Fields::Named(_) => Err(syn::Error::new_spanned(
            &input.ident,
            "FromPyObject derive for named structs is not yet supported in this shim",
        )),
        syn::Fields::Unit => Ok(quote! {
            impl<'py> ::pyo3::FromPyObject<'py> for #ident {
                fn extract_bound(_ob: &::pyo3::Bound<'py, ::pyo3::types::PyAny>) -> ::pyo3::PyResult<Self> {
                    Ok(#ident)
                }
            }
        }),
    }
}
