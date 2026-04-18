use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::Parser;
use syn::{Attribute, FnArg, ItemFn, Pat, Result, ReturnType};

pub fn expand(attr: TokenStream, mut input: ItemFn) -> Result<TokenStream> {
    let fn_name = &input.sig.ident;
    let mut fn_name_str = fn_name.to_string();
    let wrapper_name = format_ident!("__pyo3_fn_{}", fn_name);
    let symbol_wrapper_name = format_ident!("__pyo3_wrap_symbol_{}", fn_name);

    let parser = syn::meta::parser(|meta| {
        if meta.path.is_ident("name") {
            let value = meta.value()?;
            let lit: syn::LitStr = value.parse()?;
            fn_name_str = lit.value();
        } else if meta.input.peek(syn::Token![=]) {
            let value = meta.value()?;
            let _: proc_macro2::TokenStream = value.parse()?;
        } else if meta.input.peek(syn::token::Paren) {
            meta.parse_nested_meta(|_| Ok(()))?;
        }
        Ok(())
    });
    parser.parse2(attr)?;

    let mut extraction_stmts: Vec<TokenStream> = vec![];
    let mut call_exprs: Vec<TokenStream> = vec![];

    let mut arg_index: usize = 0;

    for arg in &mut input.sig.inputs {
        let FnArg::Typed(pat_type) = arg else {
            return Err(syn::Error::new_spanned(
                arg,
                "self arguments not yet supported",
            ));
        };

        let pat_name = match pat_type.pat.as_ref() {
            Pat::Ident(pi) => pi.ident.clone(),
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "unsupported argument pattern",
                ))
            }
        };

        let ty = &pat_type.ty;
        let ty_str = quote!(#ty).to_string().replace(' ', "");
        let from_py_with = take_from_py_with_attr(&mut pat_type.attrs)?;

        if ty_str.contains("Python") {
            call_exprs.push(quote! { ::pyo3::Python::from_vm(__vm) });
            continue;
        }

        let (extraction, call_expr) =
            gen_extraction(&ty_str, &pat_name, &mut arg_index, from_py_with.as_ref())?;
        extraction_stmts.push(extraction);
        call_exprs.push(call_expr);
    }

    let (ret_annotation, ret_transform) = return_handling(&input.sig.output);

    Ok(quote! {
        #input

        #[doc(hidden)]
        #[allow(non_snake_case, dead_code)]
        pub fn #wrapper_name<'__py>(
            __py: ::pyo3::Python<'__py>,
        ) -> ::pyo3::Bound<'__py, ::pyo3::types::PyAny> {
            let __vm = __py.vm();
            let __heap_def = __vm.ctx.new_method_def(
                #fn_name_str,
                {
                    fn __pyo3_impl(__args: ::rustpython_vm::function::FuncArgs, __vm: &::rustpython_vm::VirtualMachine) #ret_annotation {
                        let mut __args = __args;
                        #(#extraction_stmts)*
                        let __result = #fn_name(#(#call_exprs),*);
                        #ret_transform
                    }
                    __pyo3_impl
                },
                ::rustpython_vm::function::PyMethodFlags::empty(),
                None,
            );
            let __callable = __heap_def.build_function(__vm);
            ::pyo3::Bound::from_object(__py, __callable.into())
        }

        #[doc(hidden)]
        #[unsafe(no_mangle)]
        pub extern "Rust" fn #symbol_wrapper_name(
            __py: ::pyo3::Python<'_>,
        ) -> ::rustpython_vm::PyObjectRef {
            let __bound = #wrapper_name(__py);
            let __obj: ::rustpython_vm::PyObjectRef = __bound.into_any().into();
            __obj
        }
    })
}

fn return_handling(ret: &ReturnType) -> (TokenStream, TokenStream) {
    match ret {
        ReturnType::Default => (
            quote! { -> ::rustpython_vm::PyResult<::rustpython_vm::PyObjectRef> },
            quote! { Ok(__vm.ctx.none()) },
        ),
        ReturnType::Type(_, ty) => {
            let ty_str = quote!(#ty).to_string().replace(' ', "");
            if ty_str.contains("PyResult") {
                let inner = ty_str
                    .strip_prefix("PyResult<")
                    .and_then(|s| s.strip_suffix('>'))
                    .unwrap_or("_");
                match inner {
                    "()" => (
                        quote! { -> ::rustpython_vm::PyResult<::rustpython_vm::PyObjectRef> },
                        quote! { __result.map(|_| __vm.ctx.none()).map_err(::pyo3::err::into_vm_err) },
                    ),
                    _ => (
                        quote! { -> ::rustpython_vm::PyResult<::rustpython_vm::PyObjectRef> },
                        quote! {
                            __result
                                .and_then(|v| {
                                    ::pyo3::IntoPyObject::into_pyobject(v, ::pyo3::Python::from_vm(__vm))
                                        .map(|b| {
                                            let obj: ::rustpython_vm::PyObjectRef = b.into_any().into();
                                            obj
                                        })
                                })
                                .map_err(::pyo3::err::into_vm_err)
                        },
                    ),
                }
            } else {
                match ty_str.as_ref() {
                    "()" => (
                        quote! { -> ::rustpython_vm::PyResult<::rustpython_vm::PyObjectRef> },
                        quote! { Ok(__vm.ctx.none()) },
                    ),
                    _ => (
                        quote! { -> ::rustpython_vm::PyResult<::rustpython_vm::PyObjectRef> },
                        quote! {
                            ::pyo3::IntoPyObject::into_pyobject(__result, ::pyo3::Python::from_vm(__vm))
                                .map(|b| {
                                    let obj: ::rustpython_vm::PyObjectRef = b.into_any().into();
                                    obj
                                })
                                .map_err(::pyo3::err::into_vm_err)
                        },
                    ),
                }
            }
        }
    }
}

/// Generate extraction code for a parameter type.
///
/// Returns (extraction_stmts, call_expr):
/// - extraction_stmts: let-bindings that extract the argument from FuncArgs
/// - call_expr: the expression to pass to the original function
fn gen_extraction(
    ty_str: &str,
    name: &syn::Ident,
    arg_index: &mut usize,
    from_py_with: Option<&TokenStream>,
) -> Result<(TokenStream, TokenStream)> {
    let _idx = *arg_index;
    *arg_index += 1;
    let py_name = name.to_string();

    if let Some(from_py_with) = from_py_with {
        return Ok((
            quote! {
                let #name = #from_py_with(
                    &::pyo3::Bound::from_object(
                        ::pyo3::Python::from_vm(__vm),
                        __args.take_positional_keyword(#py_name).ok_or_else(|| {
                            __vm.new_type_error(
                                format!("missing required argument: {}", stringify!(#name))
                            )
                        })?
                    )
                ).map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e))?;
            },
            quote! { #name },
        ));
    }

    if ty_str.contains("Python") {
        *arg_index -= 1;
        return Ok((quote! {}, quote! { ::pyo3::Python::from_vm(__vm) }));
    }

    // For &Bound<'_, T> parameters — extract PyObjectRef, wrap in Bound, pass reference
    if let Some(inner) = ty_str.strip_prefix("&") {
        let inner = strip_leading_lifetime(inner);
        if inner.contains("Bound") {
            let bound_name = format_ident!("__bound_{}", name);
            return Ok((
                quote! {
                    let #name = __args.take_positional_keyword(#py_name).ok_or_else(|| {
                        __vm.new_type_error(
                            format!("missing required argument: {}", stringify!(#name))
                        )
                    })?;
                    let #bound_name = ::pyo3::Bound::from_object(
                        ::pyo3::Python::from_vm(__vm), #name
                    );
                },
                quote! { &#bound_name },
            ));
        }
        if inner.contains("Py<") {
            let inner_t: TokenStream = inner.parse().unwrap();
            return Ok((
                quote! {
                    let #name: #inner_t = ::pyo3::Py::from(
                        __args.take_positional_keyword(#py_name).ok_or_else(|| {
                            __vm.new_type_error(
                                format!("missing required argument: {}", stringify!(#name))
                            )
                        })?
                    );
                },
                quote! { &#name },
            ));
        }
        // &T where T is extractable via FromPyObject
        let t: TokenStream = inner.parse().unwrap();
        return Ok((
            quote! {
                let #name = <&#t as ::pyo3::FromPyObject<'_, '_>>::extract_bound(
                    &::pyo3::Bound::from_object(
                        ::pyo3::Python::from_vm(__vm),
                        __args.take_positional_keyword(#py_name).ok_or_else(|| {
                            __vm.new_type_error(
                                format!("missing required argument: {}", stringify!(#name))
                            )
                        })?
                    )
                ).map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e))?;
            },
            quote! { #name },
        ));
    }

    // Option<&Bound<'_, T>> — optional reference to a Bound wrapper
    if let Some(inner) = ty_str
        .strip_prefix("Option<")
        .and_then(|s| s.strip_suffix('>'))
    {
        return gen_option_extraction(inner, name, arg_index);
    }

    // Simple types
    match ty_str.as_ref() {
        "&str" => Ok((
            quote! {
                let #name: ::rustpython_vm::builtins::PyStrRef = __args.take_positional_keyword(#py_name).ok_or_else(|| {
                    __vm.new_type_error(
                        format!("missing required argument: {}", stringify!(#name))
                    )
                })?.try_into_value(__vm).map_err(|e| e)?;
            },
            quote! { #name.as_str() },
        )),
        "String" => Ok((
            quote! {
                let #name: ::rustpython_vm::builtins::PyStrRef = __args.take_positional_keyword(#py_name).ok_or_else(|| {
                    __vm.new_type_error(
                        format!("missing required argument: {}", stringify!(#name))
                    )
                })?.try_into_value(__vm).map_err(|e| e)?;
            },
            quote! { #name.to_string() },
        )),
        "i8" | "i16" | "i32" | "i64" => {
            let t: TokenStream = ty_str.parse().unwrap();
            Ok((
                quote! {
                    let #name: i64 = __args.take_positional_keyword(#py_name).ok_or_else(|| {
                        __vm.new_type_error(
                            format!("missing required argument: {}", stringify!(#name))
                        )
                    })?.try_into_value(__vm).map_err(|e| e)?;
                },
                quote! { #name as #t },
            ))
        }
        "u8" | "u16" | "u32" | "u64" | "usize" => {
            let t: TokenStream = ty_str.parse().unwrap();
            Ok((
                quote! {
                    let #name: u64 = __args.take_positional_keyword(#py_name).ok_or_else(|| {
                        __vm.new_type_error(
                            format!("missing required argument: {}", stringify!(#name))
                        )
                    })?.try_into_value(__vm).map_err(|e| e)?;
                },
                quote! { #name as #t },
            ))
        }
        "f32" | "f64" => {
            let t: TokenStream = ty_str.parse().unwrap();
            Ok((
                quote! {
                    let #name: f64 = __args.take_positional_keyword(#py_name).ok_or_else(|| {
                        __vm.new_type_error(
                            format!("missing required argument: {}", stringify!(#name))
                        )
                    })?.try_into_value(__vm).map_err(|e| e)?;
                },
                quote! { #name as #t },
            ))
        }
        "bool" => Ok((
            quote! {
                let #name: bool = __args.take_positional_keyword(#py_name).ok_or_else(|| {
                    __vm.new_type_error(
                        format!("missing required argument: {}", stringify!(#name))
                    )
                })?.try_into_value(__vm).map_err(|e| e)?;
            },
            quote! { #name },
        )),
        "Vec<u8>" => Ok((
            quote! {
                let #name: ::rustpython_vm::builtins::PyBytesRef = __args.take_positional_keyword(#py_name).ok_or_else(|| {
                    __vm.new_type_error(
                        format!("missing required argument: {}", stringify!(#name))
                    )
                })?.try_into_value(__vm).map_err(|e| e)?;
            },
            quote! { #name.as_bytes().to_vec() },
        )),
        other if other.contains("Bound") => {
            let bound_name = format_ident!("__bound_{}", name);
            Ok((
                quote! {
                    let #name = __args.take_positional_keyword(#py_name).ok_or_else(|| {
                        __vm.new_type_error(
                            format!("missing required argument: {}", stringify!(#name))
                        )
                    })?;
                    let #bound_name = ::pyo3::Bound::from_object(
                        ::pyo3::Python::from_vm(__vm), #name
                    );
                },
                quote! { #bound_name },
            ))
        }
        _ => {
            let t: TokenStream = ty_str.parse().unwrap();
            Ok((
                quote! {
                                                    let #name = match <#t as ::pyo3::FromPyObject<'_, '_>>::extract_bound(
                                                        &::pyo3::Bound::from_object(
                                                            ::pyo3::Python::from_vm(__vm),
                                                            __args.take_positional_keyword(#py_name).ok_or_else(|| {
                __vm.new_type_error(format!("missing required argument: {}", stringify!(#name)))
                                                            })?
                                                        )
                                                    ) {
                                                        Ok(v) => v,
                                                        Err(e) => return Err(::pyo3::err::into_vm_err(e)),
                                                    };
                                                },
                quote! { #name },
            ))
        }
    }
}

fn take_from_py_with_attr(attrs: &mut Vec<Attribute>) -> Result<Option<TokenStream>> {
    let mut from_py_with = None;
    attrs.retain(|attr| {
        if !attr.path().is_ident("pyo3") {
            return true;
        }
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("from_py_with") {
                let value = meta.value()?;
                let expr: syn::Expr = value.parse()?;
                let rendered = quote! { #expr }.to_string();
                let compact = rendered.replace(' ', "");
                if compact.contains("Bound<'_,_>") && compact.contains("PyAnyMethods>::len") {
                    from_py_with = Some(
                        quote! { <::pyo3::Bound<'_, ::pyo3::types::PyAny> as ::pyo3::types::PyAnyMethods>::len }
                    );
                } else {
                    from_py_with = Some(quote! { #expr });
                }
            }
            Ok(())
        });
        false
    });
    Ok(from_py_with)
}

fn strip_leading_lifetime(mut s: &str) -> &str {
    if !s.starts_with('\'') {
        return s;
    }
    let mut idx = 1usize;
    for ch in s[1..].chars() {
        if ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' {
            idx += ch.len_utf8();
        } else {
            break;
        }
    }
    s = &s[idx..];
    s.trim_start()
}

/// Generate extraction for Option<T> parameters.
fn gen_option_extraction(
    inner: &str,
    name: &syn::Ident,
    _arg_index: &mut usize,
) -> Result<(TokenStream, TokenStream)> {
    let py_name = name.to_string();
    // Option<&Bound<'_, T>>
    if let Some(bound_inner) = inner.strip_prefix("&") {
        if bound_inner.contains("Bound") {
            let bound_name = format_ident!("__bound_{}", name);
            return Ok((
                quote! {
                    let #name = __args.take_positional_keyword(#py_name);
                    let #bound_name = match #name {
                        Some(__obj) => {
                            if __vm.is_none(&__obj) {
                                None
                            } else {
                                Some(::pyo3::Bound::from_object(
                                    ::pyo3::Python::from_vm(__vm), __obj
                                ))
                            }
                        }
                        None => None,
                    };
                },
                quote! { #bound_name.as_ref() },
            ));
        }
        if bound_inner.contains("Py<") {
            let inner_t: TokenStream = bound_inner.parse().unwrap();
            return Ok((
                quote! {
                    let #name = __args.take_positional_keyword(#py_name);
                    let #name: Option<#inner_t> = match #name {
                        Some(__obj) => {
                            if __vm.is_none(&__obj) {
                                None
                            } else {
                                Some(::pyo3::Py::from(__obj))
                            }
                        }
                        None => None,
                    };
                },
                quote! { #name.as_ref() },
            ));
        }
        // Option<&T> where T is extractable
        let t: TokenStream = bound_inner.parse().unwrap();
        return Ok((
            quote! {
                let #name = __args.take_positional_keyword(#py_name);
                let #name: Option<&#t> = match #name {
                    Some(__obj) => {
                        if __vm.is_none(&__obj) {
                            None
                        } else {
                            Some(<&#t as ::pyo3::FromPyObject<'_, '_>>::extract_bound(
                                &::pyo3::Bound::from_object(::pyo3::Python::from_vm(__vm), __obj)
                            ).map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e))?)
                        }
                    }
                    None => None,
                };
            },
            quote! { #name },
        ));
    }

    // Option<primitive>
    match inner {
        "u8" => Ok((
            quote! {
                let #name = __args.take_positional_keyword(#py_name);
                let #name: Option<u8> = match #name {
                    Some(__obj) => {
                        if __vm.is_none(&__obj) { None }
                        else { Some(__obj.try_into_value(__vm).map_err(|e| e)?) }
                    }
                    None => None,
                };
            },
            quote! { #name },
        )),
        "u16" => Ok((
            quote! {
                let #name = __args.take_positional_keyword(#py_name);
                let #name: Option<u16> = match #name {
                    Some(__obj) => {
                        if __vm.is_none(&__obj) { None }
                        else { Some(__obj.try_into_value(__vm).map_err(|e| e)?) }
                    }
                    None => None,
                };
            },
            quote! { #name },
        )),
        "u32" => Ok((
            quote! {
                let #name = __args.take_positional_keyword(#py_name);
                let #name: Option<u32> = match #name {
                    Some(__obj) => {
                        if __vm.is_none(&__obj) { None }
                        else { Some(__obj.try_into_value(__vm).map_err(|e| e)?) }
                    }
                    None => None,
                };
            },
            quote! { #name },
        )),
        "u64" | "usize" => {
            let t: TokenStream = inner.parse().unwrap();
            Ok((
                quote! {
                    let #name = __args.take_positional_keyword(#py_name);
                    let #name: Option<#t> = match #name {
                        Some(__obj) => {
                            if __vm.is_none(&__obj) { None }
                            else { Some(__obj.try_into_value(__vm).map_err(|e| e)?) }
                        }
                        None => None,
                    };
                },
                quote! { #name },
            ))
        }
        "i8" | "i16" | "i32" | "i64" => {
            let t: TokenStream = inner.parse().unwrap();
            Ok((
                quote! {
                    let #name = __args.take_positional_keyword(#py_name);
                    let #name: Option<#t> = match #name {
                        Some(__obj) => {
                            if __vm.is_none(&__obj) { None }
                            else { Some(__obj.try_into_value(__vm).map_err(|e| e)?) }
                        }
                        None => None,
                    };
                },
                quote! { #name },
            ))
        }
        "f32" | "f64" => {
            let t: TokenStream = inner.parse().unwrap();
            Ok((
                quote! {
                    let #name = __args.take_positional_keyword(#py_name);
                    let #name: Option<#t> = match #name {
                        Some(__obj) => {
                            if __vm.is_none(&__obj) { None }
                            else { Some(__obj.try_into_value(__vm).map_err(|e| e)?) }
                        }
                        None => None,
                    };
                },
                quote! { #name },
            ))
        }
        "bool" => Ok((
            quote! {
                let #name = __args.take_positional_keyword(#py_name);
                let #name: Option<bool> = match #name {
                    Some(__obj) => {
                        if __vm.is_none(&__obj) { None }
                        else { Some(__obj.try_into_value(__vm).map_err(|e| e)?) }
                    }
                    None => None,
                };
            },
            quote! { #name },
        )),
        "String" => Ok((
            quote! {
                let #name = __args.take_positional_keyword(#py_name);
                let #name: Option<String> = match #name {
                    Some(__obj) => {
                        if __vm.is_none(&__obj) { None }
                        else {
                            let __s: ::rustpython_vm::builtins::PyStrRef = __obj.try_into_value(__vm).map_err(|e| e)?;
                            Some(__s.to_string())
                        }
                    }
                    None => None,
                };
            },
            quote! { #name },
        )),
        "&str" => {
            let temp_name = format_ident!("__temp_{}", name);
            Ok((
                quote! {
                    let #name = __args.take_positional_keyword(#py_name);
                    let #temp_name: Option<::rustpython_vm::builtins::PyStrRef> = match #name {
                        Some(__obj) => {
                            if __vm.is_none(&__obj) { None }
                            else { Some(__obj.try_into_value(__vm).map_err(|e| e)?) }
                        }
                        None => None,
                    };
                    let #name: Option<&str> = #temp_name.as_ref().map(|s| s.as_str());
                },
                quote! { #name },
            ))
        }
        other if other.contains("Bound") => {
            let bound_name = format_ident!("__bound_{}", name);
            Ok((
                quote! {
                    let #name = __args.take_positional_keyword(#py_name);
                    let #bound_name = match #name {
                        Some(__obj) => {
                            if __vm.is_none(&__obj) { None }
                            else { Some(::pyo3::Bound::from_object(
                                ::pyo3::Python::from_vm(__vm), __obj
                            )) }
                        }
                        None => None,
                    };
                },
                quote! { #bound_name },
            ))
        }
        _ => {
            let t: TokenStream = inner.parse().unwrap();
            Ok((
                quote! {
                    let #name = __args.take_positional_keyword(#py_name);
                    let #name: Option<#t> = match #name {
                        Some(__obj) => {
                            if __vm.is_none(&__obj) { None }
                            else {
                                match <#t as ::pyo3::FromPyObject<'_, '_>>::extract_bound(
                                    &::pyo3::Bound::from_object(::pyo3::Python::from_vm(__vm), __obj)
                                ) {
                                    Ok(v) => Some(v),
                                    Err(e) => return Err(::pyo3::err::into_vm_err(e)),
                                }
                            }
                        }
                        None => None,
                    };
                },
                quote! { #name },
            ))
        }
    }
}
