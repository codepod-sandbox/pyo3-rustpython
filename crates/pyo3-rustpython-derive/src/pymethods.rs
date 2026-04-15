use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    token::Comma, Expr, FnArg, ImplItem, ImplItemConst, ImplItemFn, ItemImpl, Pat, Result,
    ReturnType, Token,
};

pub fn expand(_attr: TokenStream, input: ItemImpl) -> Result<TokenStream> {
    let self_ty = &input.self_ty;
    let inventory_class_name = inventory_class_name(self_ty)?;

    let mut has_new = false;
    let mut new_method: Option<ImplItemFn> = None;
    let mut transformed_items: Vec<TokenStream> = Vec::new();
    let mut outer_items: Vec<TokenStream> = Vec::new();
    let mut classattr_consts: Vec<TokenStream> = Vec::new();

    for item in &input.items {
        let ImplItem::Fn(method) = item else {
            if let ImplItem::Const(const_item) = item {
                if has_attr(&const_item.attrs, "classattr") {
                    let cleaned = strip_pyo3_const_attrs(const_item);
                    classattr_consts.push(quote! { #cleaned });
                    continue;
                }
            }

            transformed_items.push(quote! { #item });
            continue;
        };

        let fn_name_str = method.sig.ident.to_string();

        if has_attr(&method.attrs, "new") {
            has_new = true;
            new_method = Some(method.clone());
            let cleaned = strip_pyo3_method_attrs(method);
            transformed_items.push(quote! { #cleaned });
            continue;
        }

        if has_attr(&method.attrs, "classattr") {
            let cleaned = strip_pyo3_method_attrs(method);
            transformed_items.push(quote! { #cleaned });
            continue;
        }

        if has_attr(&method.attrs, "getter") {
            let cleaned = strip_pyo3_method_attrs(method);
            if getter_needs_wrapper(&cleaned) {
                let wrapper = generate_getter_wrapper(&cleaned, self_ty);
                transformed_items.push(wrapper);
            } else {
                transformed_items.push(quote! {
                    #[pygetset]
                    #cleaned
                });
            }
            continue;
        }

        if has_attr(&method.attrs, "setter") {
            let cleaned = strip_pyo3_method_attrs(method);
            if setter_needs_wrapper(&cleaned) {
                let wrapper = generate_setter_wrapper(&cleaned, self_ty);
                transformed_items.push(wrapper);
            } else {
                transformed_items.push(quote! {
                    #[pygetset(setter)]
                    #cleaned
                });
            }
            continue;
        }

        if has_attr(&method.attrs, "staticmethod") {
            let cleaned = strip_pyo3_method_attrs(method);
            if needs_wrapper(&cleaned) {
                let wrapper = generate_staticmethod_wrapper(&cleaned, self_ty)?;
                transformed_items.push(wrapper);
            } else {
                transformed_items.push(quote! {
                    #[pystaticmethod]
                    #cleaned
                });
            }
            continue;
        }

        if has_attr(&method.attrs, "classmethod") {
            let cleaned = strip_pyo3_method_attrs(method);
            if needs_wrapper(&cleaned) {
                let wrapper = generate_classmethod_wrapper(&cleaned, self_ty)?;
                transformed_items.push(wrapper);
            } else {
                transformed_items.push(quote! {
                    #[pyclassmethod]
                    #cleaned
                });
            }
            continue;
        }

        let is_slot = is_rustpython_slot_method(&fn_name_str);
        let cleaned = strip_pyo3_method_attrs(method);

        if is_slot {
            if fn_name_str == "__iter__" {
                let wrapper = generate_iter_wrapper(&cleaned, self_ty);
                transformed_items.push(wrapper);
            } else if fn_name_str == "__next__" {
                let wrapper = generate_next_wrapper(&cleaned, self_ty);
                transformed_items.push(wrapper);
            } else {
                let slot_name = format_ident!("_pyo3_slot_{}", fn_name_str);
                let (wrapper, helper_item) =
                    generate_slot_method_wrapper(&cleaned, self_ty, &slot_name);
                transformed_items.push(wrapper);
                if let Some(helper_item) = helper_item {
                    outer_items.push(helper_item);
                }
            }
            continue;
        }

        if needs_wrapper(&cleaned) {
            let wrapper = generate_pyresult_wrapper(&cleaned, self_ty);
            transformed_items.push(wrapper);
        } else {
            transformed_items.push(quote! {
                #[pymethod]
                #cleaned
            });
        }
    }

    let pyclass_attr = if has_new {
        quote! { #[::rustpython_vm::pyclass(payload = "__Pyo3MethodHelper", with(::rustpython_vm::types::Constructor))] }
    } else {
        quote! { #[::rustpython_vm::pyclass(payload = "__Pyo3MethodHelper")] }
    };

    let constructor_impl = if let Some(ref new_fn) = new_method {
        Some(generate_constructor_impl(new_fn, self_ty)?)
    } else {
        None
    };

    let classattr_impl = if classattr_consts.is_empty() {
        None
    } else {
        Some(quote! {
            impl #self_ty {
                #(#classattr_consts)*
            }
        })
    };

    Ok(quote! {
        #pyclass_attr
        impl #self_ty {
            #(#transformed_items)*
        }

        ::pyo3::inventory::submit! {
            #inventory_class_name::new(::pyo3::Pyo3ClassItems {
                methods: #self_ty::__OWN_METHOD_DEFS,
                extend_class: |ctx, class| {
                    #self_ty::__extend_py_class(
                        unsafe {
                            ::std::mem::transmute::<
                                &::rustpython_vm::Context,
                                &'static ::rustpython_vm::Context,
                            >(ctx)
                        },
                        class,
                    )
                },
                extend_slots: #self_ty::__extend_slots,
            })
        }

        #constructor_impl
        #classattr_impl
        #(#outer_items)*
    })
}

fn inventory_class_name(self_ty: &syn::Type) -> Result<syn::Ident> {
    let base = self_ty_ident(self_ty).ok_or_else(|| {
        syn::Error::new_spanned(self_ty, "#[pymethods] currently requires a simple type path")
    })?;
    Ok(format_ident!("__Pyo3InventoryFor{}", base))
}

fn self_ty_ident(self_ty: &syn::Type) -> Option<syn::Ident> {
    match self_ty {
        syn::Type::Path(type_path) => type_path.path.segments.last().map(|segment| segment.ident.clone()),
        _ => None,
    }
}

// ─── Constructor ──────────────────────────────────────────────────────────────

fn generate_constructor_impl(new_fn: &ImplItemFn, self_ty: &syn::Type) -> Result<TokenStream> {
    let fn_name = &new_fn.sig.ident;
    let inner_name = format_ident!("_pyo3_inner_{}", fn_name);
    let mut inner_fn = strip_pyo3_method_attrs(new_fn);
    inner_fn.sig.ident = inner_name.clone();
    let ret_str = return_type_string(&new_fn.sig.output);
    let signature_defaults = parse_constructor_signature_defaults(&new_fn.attrs)?;

    let returns_result = ret_str.contains("PyResult");
    let returns_tuple = ret_str.contains("Self,") || ret_str.contains("Self ,");
    let returns_result_tuple = returns_result && returns_tuple;

    let (extract_base, _base_ty) = if returns_tuple || returns_result_tuple {
        extract_base_type_from_return(&new_fn.sig.output)
    } else {
        (false, None)
    };

    let (py_extraction, py_call_args) = generate_funcargs_extraction(new_fn)?;

    let py_new_call_expr = if let Some(signature_defaults) = signature_defaults {
        let mut helper_fields = Vec::new();
        let mut helper_field_names = Vec::new();
        let mut binding_stmts = Vec::new();
        let mut call_args = Vec::new();
        let mut helper_defaults = signature_defaults.into_iter();
        let mut required_count = 0usize;

        for arg in &new_fn.sig.inputs {
            let FnArg::Typed(pt) = arg else {
                continue;
            };

            let ty = &pt.ty;
            let ty_str = quote!(#ty).to_string().replace(' ', "");
            if ty_str.contains("Python") {
                continue;
            }

            let name = match pt.pat.as_ref() {
                Pat::Ident(pi) => pi.ident.clone(),
                other => {
                    return Err(syn::Error::new_spanned(
                        other,
                        "unsupported argument pattern in #[new]",
                    ))
                }
            };

            let default = helper_defaults.next().flatten();
            let py_name = name.to_string();
            if let Some(default) = default {
                let default_raw = if is_none_expr(&default) {
                    quote! { vm.ctx.none() }
                } else {
                    quote! { ::rustpython_vm::convert::ToPyObject::to_pyobject(#default, vm) }
                };
                binding_stmts.push(quote! {
                    let #name = args
                        .take_positional_keyword(#py_name)
                        .unwrap_or_else(|| #default_raw);
                });
            } else {
                required_count += 1;
                binding_stmts.push(quote! {
                    let #name = args
                        .take_positional_keyword(#py_name)
                        .ok_or(::rustpython_vm::function::ArgumentError::TooFewArgs)?;
                });
            }

            helper_fields.push(quote! {
                #name: ::rustpython_vm::PyObjectRef,
            });
            helper_field_names.push(name.clone());
            call_args.push(quote! { __new_args.#name });
        }

        let helper_name = format_ident!("_Pyo3NewArgs_{}", fn_name);
        let helper_total_count = helper_field_names.len();

        quote! {{
            #[allow(non_camel_case_types, dead_code)]
            struct #helper_name {
                #(#helper_fields)*
            }

            impl ::rustpython_vm::function::FromArgs for #helper_name {
                fn arity() -> ::std::ops::RangeInclusive<usize> {
                    #required_count..=#helper_total_count
                }

                fn from_args(
                    vm: &::rustpython_vm::VirtualMachine,
                    args: &mut ::rustpython_vm::function::FuncArgs,
                ) -> ::core::result::Result<Self, ::rustpython_vm::function::ArgumentError> {
                    #(#binding_stmts)*
                    Ok(Self {
                        #(#helper_field_names),*
                    })
                }
            }

            let __new_args: #helper_name = _args.bind(_vm)?;
            let mut __args = ::rustpython_vm::function::FuncArgs::from(vec![
                #(#call_args),*
            ]);
            #py_extraction
            Self::#inner_name(#py_call_args)
        }}
    } else {
        quote! {{
            let mut __args = _args;
            #py_extraction
            Self::#inner_name(#py_call_args)
        }}
    };

    let body = match (returns_result, extract_base) {
        (false, false) => quote! { Ok((#py_new_call_expr)) },
        (true, false) => {
            quote! { (#py_new_call_expr).map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e)) }
        }
        (false, true) => quote! { Ok((#py_new_call_expr).0) },
        (true, true) => {
            quote! { (#py_new_call_expr).map(|t| t.0).map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e)) }
        }
    };

    let slot_new = if extract_base {
        let (_, base_ty) = extract_base_type_from_return(&new_fn.sig.output);
        if base_ty.is_none() {
            return Err(syn::Error::new_spanned(
                &new_fn.sig.output,
                "expected #[new] returning (Self, Base) or PyResult<(Self, Base)>",
            ));
        }

        let slot_body = if returns_result {
            quote! {{
                let (_, base_payload): (
                    Self,
                    <Self as ::pyo3::Pyo3BasePayload>::BasePayload,
                ) = #py_new_call_expr
                    .map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e))?;
                let dict = if cls
                    .slots
                    .flags
                    .has_feature(::rustpython_vm::types::PyTypeFlags::HAS_DICT)
                {
                    Some(vm.ctx.new_dict())
                } else {
                    None
                };
                Ok(::rustpython_vm::PyRef::new_ref(base_payload, cls, dict).into())
            }}
        } else {
            quote! {{
                let (_, base_payload): (
                    Self,
                    <Self as ::pyo3::Pyo3BasePayload>::BasePayload,
                ) = #py_new_call_expr;
                let dict = if cls
                    .slots
                    .flags
                    .has_feature(::rustpython_vm::types::PyTypeFlags::HAS_DICT)
                {
                    Some(vm.ctx.new_dict())
                } else {
                    None
                };
                Ok(::rustpython_vm::PyRef::new_ref(base_payload, cls, dict).into())
            }}
        };

        Some(quote! {
            fn slot_new(
                cls: ::rustpython_vm::builtins::PyTypeRef,
                _args: Self::Args,
                vm: &::rustpython_vm::VirtualMachine,
            ) -> ::rustpython_vm::PyResult {
                let _vm = vm;
                #slot_body
            }
        })
    } else {
        None
    };

    Ok(quote! {
        impl #self_ty {
            #[allow(dead_code)]
            #inner_fn
        }

        impl ::rustpython_vm::types::Constructor for #self_ty {
            type Args = ::rustpython_vm::function::FuncArgs;

            fn py_new(
                _cls: &::rustpython_vm::Py<::rustpython_vm::builtins::PyType>,
                _args: Self::Args,
                vm: &::rustpython_vm::VirtualMachine,
            ) -> ::rustpython_vm::PyResult<Self> {
                let _vm = vm;
                #body
            }

            #slot_new
        }
    })
}

fn extract_base_type_from_return(ret: &ReturnType) -> (bool, Option<syn::Path>) {
    let s = match ret {
        ReturnType::Default => return (false, None),
        ReturnType::Type(_, ty) => quote!(#ty).to_string().replace(' ', ""),
    };

    let inner = if s.contains("PyResult") {
        if let Some(start) = s.find('<') {
            if let Some(end) = s.rfind('>') {
                &s[start + 1..end]
            } else {
                return (false, None);
            }
        } else {
            return (false, None);
        }
    } else {
        &s
    };

    if !inner.contains("Self,") && !inner.contains("Self ,") {
        return (false, None);
    }

    if let Some(paren_start) = inner.find('(') {
        if let Some(paren_end) = inner.rfind(')') {
            let tuple_inner = &inner[paren_start + 1..paren_end];
            let parts: Vec<&str> = tuple_inner.split(',').collect();
            if parts.len() == 2 {
                let base_str = parts[1].trim();
                if let Ok(path) = syn::parse_str::<syn::Path>(base_str) {
                    return (true, Some(path));
                }
            }
        }
    }

    (false, None)
}

// ─── Getter/Setters ───────────────────────────────────────────────────────────

fn getter_needs_wrapper(method: &ImplItemFn) -> bool {
    if has_vm_virtualmachine_param(method) {
        return true;
    }

    if has_pyo3_params(method) {
        return true;
    }

    let ret_str = return_type_string(&method.sig.output);
    if ret_str.starts_with("&str") || ret_str.starts_with("&'") {
        return true;
    }
    if ret_str.contains("Py<") || ret_str.contains("PyResult") {
        return true;
    }

    false
}

fn setter_needs_wrapper(method: &ImplItemFn) -> bool {
    has_pyo3_params(method) || has_vm_virtualmachine_param(method)
}

fn generate_getter_wrapper(method: &ImplItemFn, self_ty: &syn::Type) -> TokenStream {
    let fn_name = &method.sig.ident;
    let wrapper_name = format_ident!("_pyo3_getter_{}", fn_name);
    let vis = syn::Visibility::Inherited;
    let generics = &method.sig.generics;
    let fn_name_str = fn_name.to_string();

    let has_py_param = method.sig.inputs.iter().any(|arg| {
        if let FnArg::Typed(pt) = arg {
            let s = quote!(#pt).to_string().replace(' ', "");
            s.contains("Python")
        } else {
            false
        }
    });

    let inner_body = &method.block;
    let inner_ret = &method.sig.output;
    let inner_params: Vec<_> = method.sig.inputs.iter().filter(|a| !matches!(a, FnArg::Receiver(_))).collect();

    let ret_str = return_type_string(&method.sig.output);
    let returns_pyo3_result = returns_pyo3_result(&method.sig.output);
    let returns_rustpython_result = ret_str.contains("rustpython_vm::PyResult");
    let has_vm_param = has_vm_virtualmachine_param(method);

    let body = if has_py_param {
        if returns_pyo3_result {
            quote! {
                match self.#fn_name(__py) {
                    Ok(val) => ::rustpython_vm::convert::ToPyObject::to_pyobject(val, _vm),
                    Err(e) => return Err(::pyo3::err::into_vm_err(e)),
                }
            }
        } else if returns_rustpython_result {
            quote! {
                match self.#fn_name(__py) {
                    Ok(val) => ::rustpython_vm::convert::ToPyObject::to_pyobject(val, _vm),
                    Err(e) => return Err(e),
                }
            }
        } else if ret_str.contains("Py<") {
            quote! {
                ::rustpython_vm::convert::IntoObject::into_object(self.#fn_name(__py))
            }
        } else {
            quote! {
                ::rustpython_vm::convert::ToPyObject::to_pyobject(self.#fn_name(__py), _vm)
            }
        }
    } else if has_vm_param {
        if returns_pyo3_result {
            quote! {
                match self.#fn_name(_vm) {
                    Ok(val) => ::rustpython_vm::convert::ToPyObject::to_pyobject(val, _vm),
                    Err(e) => return Err(::pyo3::err::into_vm_err(e)),
                }
            }
        } else if returns_rustpython_result {
            quote! {
                match self.#fn_name(_vm) {
                    Ok(val) => ::rustpython_vm::convert::ToPyObject::to_pyobject(val, _vm),
                    Err(e) => return Err(e),
                }
            }
        } else if ret_str.contains("Py<") {
            quote! {
                ::rustpython_vm::convert::IntoObject::into_object(self.#fn_name(_vm))
            }
        } else {
            quote! {
                ::rustpython_vm::convert::ToPyObject::to_pyobject(self.#fn_name(_vm), _vm)
            }
        }
    } else if ret_str.starts_with("&str") {
        quote! {
            ::rustpython_vm::convert::ToPyObject::to_pyobject(self.#fn_name().to_string(), _vm)
        }
    } else if returns_pyo3_result {
        quote! {
            match self.#fn_name() {
                Ok(val) => ::rustpython_vm::convert::ToPyObject::to_pyobject(val, _vm),
                Err(e) => return Err(::pyo3::err::into_vm_err(e)),
            }
        }
    } else if returns_rustpython_result {
        quote! {
            match self.#fn_name() {
                Ok(val) => ::rustpython_vm::convert::ToPyObject::to_pyobject(val, _vm),
                Err(e) => return Err(e),
            }
        }
    } else if ret_str.contains("Py<") {
        quote! {
            ::rustpython_vm::convert::IntoObject::into_object(self.#fn_name())
        }
    } else {
        quote! {
            ::rustpython_vm::convert::ToPyObject::to_pyobject(self.#fn_name(), _vm)
        }
    };

    quote! {
        #[pygetset(name = #fn_name_str)]
        #vis fn #wrapper_name #generics(&self, vm: &::rustpython_vm::VirtualMachine) -> ::rustpython_vm::PyResult<::rustpython_vm::PyObjectRef> {
            let _vm = vm;
            let __py = ::pyo3::Python::from_vm(_vm);
            let py = __py;
            let vm = _vm;
            Ok(#body)
        }

        #[allow(dead_code)]
        #vis fn #fn_name #generics(&self, #(#inner_params),*) #inner_ret #inner_body
    }
}
fn generate_setter_wrapper(method: &ImplItemFn, self_ty: &syn::Type) -> TokenStream {
    let fn_name = &method.sig.ident;
    let wrapper_name = format_ident!("_pyo3_setter_{}", fn_name);
    let vis = syn::Visibility::Inherited;
    let generics = &method.sig.generics;
    let fn_name_str = fn_name.to_string();

    let inner_body = &method.block;
    let inner_ret = &method.sig.output;
    let inner_params: Vec<_> = method
        .sig
        .inputs
        .iter()
        .filter(|a| !matches!(a, FnArg::Receiver(_)))
        .collect();

    let ret_str = return_type_string(&method.sig.output);
    let returns_result = ret_str.contains("PyResult");

    let err_handling = if returns_result {
        quote! { .map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e))?; }
    } else {
        quote! { ; }
    };

    quote! {
        #[pygetset(setter, name = #fn_name_str)]
        #vis fn #wrapper_name #generics(&self, value: ::rustpython_vm::function::PySetterValue<::rustpython_vm::PyObjectRef>, vm: &::rustpython_vm::VirtualMachine) {
            let _vm = vm;
            let __py = ::pyo3::Python::from_vm(_vm);
            let py = __py;
            let vm = _vm;
            let value = match value {
                ::rustpython_vm::function::PySetterValue::Assign(v) => v,
                ::rustpython_vm::function::PySetterValue::Delete => {
                    return;
                }
            };
            Self::#fn_name(self, value, _vm) #err_handling
        }

        #[allow(dead_code)]
        #vis fn #fn_name #generics(&self, #(#inner_params),*) #inner_ret #inner_body
    }
}

// ─── Slot Methods ──────────────────────────────────────────────────────────────

fn generate_slot_method_wrapper(
    method: &ImplItemFn,
    self_ty: &syn::Type,
    slot_name: &syn::Ident,
) -> (TokenStream, Option<TokenStream>) {
    let fn_name = &method.sig.ident;
    let vis = syn::Visibility::Inherited;
    let wrapper_name = format_ident!("__pyo3_wrap_{}", fn_name);
    let generics = &method.sig.generics;
    let slot_name_str = slot_name.to_string();
    let attrs = &method.attrs;
    let inner_body = &method.block;
    let inner_ret = &method.sig.output;
    let typed_result_ty = match inner_ret {
        ReturnType::Type(_, ty) => quote! { #ty },
        ReturnType::Default => quote! { () },
    };

    let mut wrapper_params: Vec<TokenStream> = Vec::new();
    let mut conversion_stmts: Vec<TokenStream> = Vec::new();
    let mut inner_call_args: Vec<TokenStream> = Vec::new();
    let helper_method = if method
        .sig
        .inputs
        .first()
        .is_some_and(|arg| matches!(arg, FnArg::Receiver(_)))
    {
        Some(strip_pyo3_method_attrs(method))
    } else {
        None
    };

    let typed_receiver = method.sig.inputs.first().and_then(|arg| match arg {
        FnArg::Typed(pt) => {
            let ty = &pt.ty;
            let ty_str = quote!(#ty).to_string().replace(' ', "");
            let name = match pt.pat.as_ref() {
                Pat::Ident(pi) => pi.ident.clone(),
                _ => return None,
            };
            if ty_str.contains("PyRefMut<") {
                Some((name, true))
            } else if ty_str.contains("PyRef<") {
                Some((name, false))
            } else {
                None
            }
        }
        FnArg::Receiver(_) => None,
    });

    for (idx, arg) in method.sig.inputs.iter().enumerate() {
        match arg {
            FnArg::Receiver(_) => continue,
            FnArg::Typed(pt) => {
                if idx == 0 && typed_receiver.is_some() {
                    continue;
                }
                let ty = &pt.ty;
                let ty_str = quote!(#ty).to_string().replace(' ', "");
                let name = if let Pat::Ident(pi) = pt.pat.as_ref() {
                    pi.ident.clone()
                } else {
                    continue;
                };
                let raw_name = format_ident!("__raw_{}", name);

                if ty_str.contains("Python") {
                    conversion_stmts.push(quote! {
                        let #name = __py;
                    });
                    inner_call_args.push(quote! { #name });
                } else if ty_str.contains("VirtualMachine") {
                    conversion_stmts.push(quote! {
                        let #name = _vm;
                    });
                    inner_call_args.push(quote! { #name });
                } else if ty_str == "&Self" || ty_str.ends_with("::Self") {
                    wrapper_params.push(quote! { #raw_name: ::rustpython_vm::PyObjectRef });
                    conversion_stmts.push(quote! {
                        let __bound_self = ::pyo3::Bound::from_object(__py, #raw_name);
                        let #name: &#self_ty = match <&#self_ty as ::pyo3::FromPyObject>::extract_bound(&__bound_self) {
                            Ok(v) => v,
                            Err(e) => return Err(::pyo3::err::into_vm_err(e)),
                        };
                    });
                    inner_call_args.push(quote! { #name });
                } else if ty_str.contains("Bound<") || ty_str.contains("&Bound<") {
                    let (extraction, call_expr) =
                        gen_bound_extraction(&ty_str, &name, &raw_name);
                    wrapper_params.push(quote! { #raw_name: ::rustpython_vm::PyObjectRef });
                    conversion_stmts.push(extraction);
                    conversion_stmts.push(quote! {
                        let #name: #ty = #call_expr;
                    });
                    inner_call_args.push(quote! { #name });
                } else if ty_str.starts_with("Option<&") {
                    let (extraction, call_expr) = gen_option_bound_extraction(&ty_str, &name);
                    wrapper_params.push(quote! { #name: ::rustpython_vm::PyObjectRef });
                    conversion_stmts.push(extraction);
                    conversion_stmts.push(quote! {
                        let #name: #ty = #call_expr;
                    });
                    inner_call_args.push(quote! { #name });
                } else if ty_str.starts_with("Option<")
                    && !ty_str.contains("Bound")
                    && !ty_str.contains("Py<")
                {
                    let (extraction, call_expr) = gen_option_extraction(&ty_str, &name, &raw_name);
                    wrapper_params.push(quote! { #raw_name: ::rustpython_vm::PyObjectRef });
                    conversion_stmts.push(extraction);
                    conversion_stmts.push(quote! {
                        let #name: #ty = #call_expr;
                    });
                    inner_call_args.push(quote! { #name });
                } else {
                    let t: TokenStream = ty_str.parse().unwrap();
                    wrapper_params.push(quote! { #raw_name: ::rustpython_vm::PyObjectRef });
                    conversion_stmts.push(quote! {
                        let #name: #t = match <#t as ::pyo3::FromPyObject>::extract_bound(
                            &::pyo3::Bound::from_object(__py, #raw_name.clone())
                        ) {
                            Ok(v) => v,
                            Err(e) => return Err(::pyo3::err::into_vm_err(e)),
                        };
                    });
                    inner_call_args.push(quote! { #name });
                }
            }
        }
    }

    let mut_receiver = typed_receiver
        .as_ref()
        .map(|(_, is_mut)| *is_mut)
        .or_else(|| {
            method.sig.inputs.first().and_then(|a| {
                if let FnArg::Receiver(r) = a {
                    Some(r.mutability.is_some())
                } else {
                    None
                }
            })
        })
        .unwrap_or(false);

    let wrapper_receiver = if typed_receiver.is_some() {
        Some(quote! { __slf: &::rustpython_vm::Py<Self> })
    } else {
        method.sig.inputs.first().and_then(|a| {
            if let FnArg::Receiver(r) = a {
                if r.mutability.is_some() {
                    Some(quote! { __slf: &::rustpython_vm::Py<Self> })
                } else {
                    Some(quote! { #r })
                }
            } else {
                None
            }
        })
    };
    let wrapper_receiver = wrapper_receiver.unwrap_or(quote! { &self });
    let has_receiver = typed_receiver.is_some()
        || method
            .sig
            .inputs
            .iter()
            .any(|a| matches!(a, FnArg::Receiver(_)));
    let ret_handling = convert_return_to_pyobj(&method.sig.output, returns_pyo3_result(&method.sig.output));
    let receiver_setup = if has_receiver {
        if mut_receiver {
            let receiver_name = typed_receiver
                .as_ref()
                .map(|(name, _)| quote! { #name })
                .unwrap_or_else(|| quote! { __pyo3_slf });
            quote! {
                let mut #receiver_name = ::pyo3::PyRefMut::from_vm_ref(__py, __slf.to_owned());
            }
        } else if let Some((receiver_name, _)) = &typed_receiver {
            quote! {
                let #receiver_name = ::pyo3::PyRef::from_vm_ref(__py, __slf.to_owned());
            }
        } else {
            quote! {}
        }
    } else {
        quote! {}
    };
    let mutable_receiver_target = typed_receiver
        .as_ref()
        .map(|(name, _)| quote! { #name })
        .unwrap_or_else(|| quote! { __pyo3_slf });

    let call_expr = if typed_receiver.is_some() {
        quote! {{
            #receiver_setup
            #(#conversion_stmts)*
            let __result: #typed_result_ty = { #inner_body };
            __result
        }}
    } else if has_receiver {
        if mut_receiver {
            quote! {{
                #receiver_setup
                #(#conversion_stmts)*
                let __result = #mutable_receiver_target.#fn_name(#(#inner_call_args),*);
                __result
            }}
        } else {
            quote! {{
                #receiver_setup
                #(#conversion_stmts)*
                let __result = self.#fn_name(#(#inner_call_args),*);
                __result
            }}
        }
    } else {
        quote! {{
            #(#conversion_stmts)*
            let __result = Self::#fn_name(#(#inner_call_args),*);
            __result
        }}
    };

    let wrapper = quote! {
        #(#attrs)*
        #[pymethod(name = #slot_name_str)]
        #vis fn #wrapper_name #generics(#wrapper_receiver, #(#wrapper_params,)* vm: &::rustpython_vm::VirtualMachine) -> ::rustpython_vm::PyResult<::rustpython_vm::PyObjectRef> {
            let _vm = vm;
            let __py = ::pyo3::Python::from_vm(_vm);
            let py = __py;
            let vm = _vm;
            let __result = #call_expr;
            #ret_handling
        }
    };

    let helper_item = helper_method.map(|helper_method| {
        quote! {
            impl #self_ty {
                #[allow(dead_code)]
                #helper_method
            }
        }
    });

    (wrapper, helper_item)
}
fn generate_slot_alias(
    self_ty: &syn::Type,
    dunder_name: &str,
    slot_name: &syn::Ident,
) -> TokenStream {
    quote! {
        {
            let typ = <#self_ty as ::rustpython_vm::class::StaticType>::static_type();
            if let Some(slot_fn) = typ.attributes.read().get(::rustpython_vm::identifier!(ctx, #slot_name)).cloned() {
                typ.set_str_attr(#dunder_name, slot_fn, ctx);
            }
            drop(typ.attributes.read());
        }
    }
}

fn generate_iter_wrapper(method: &ImplItemFn, self_ty: &syn::Type) -> TokenStream {
    let fn_name = &method.sig.ident;
    let vis = syn::Visibility::Inherited;
    let generics = &method.sig.generics;
    let wrapper_name = format_ident!("__pyo3_wrap_{}", fn_name);
    let fn_name_str = fn_name.to_string();
    let ret_str = return_type_string(&method.sig.output);
    let call_expr = match method.sig.inputs.first() {
        Some(FnArg::Receiver(_)) => quote! { slf.#fn_name() },
        _ => quote! { Self::#fn_name(slf) },
    };
    let ret_handling = convert_non_result_return_to_pyobj(&method.sig.output);
    let body = if ret_str.contains("&Self") || ret_str.contains("PyRef<") {
        quote! { Ok(slf.to_owned().into()) }
    } else {
        quote! {
            let __result = #call_expr;
            #ret_handling
        }
    };
    quote! {
        #[pymethod(name = #fn_name_str)]
        #vis fn #wrapper_name #generics(slf: &::rustpython_vm::Py<#self_ty>, vm: &::rustpython_vm::VirtualMachine) -> ::rustpython_vm::PyResult<::rustpython_vm::PyObjectRef> {
            let _vm = vm;
            let __py = ::pyo3::Python::from_vm(_vm);
            let vm = _vm;
            #body
        }

        #[allow(dead_code)]
        #method
    }
}

fn generate_next_wrapper(method: &ImplItemFn, self_ty: &syn::Type) -> TokenStream {
    let fn_name = &method.sig.ident;
    let vis = syn::Visibility::Inherited;
    let generics = &method.sig.generics;
    let wrapper_name = format_ident!("__pyo3_wrap_{}", fn_name);
    let fn_name_str = fn_name.to_string();
    let inner_body = &method.block;
    let inner_ret = &method.sig.output;
    let call_expr = match method.sig.inputs.first() {
        Some(FnArg::Receiver(_)) => quote! { slf.#fn_name() },
        _ => quote! { Self::#fn_name(slf) },
    };
    quote! {
        #[pymethod(name = #fn_name_str)]
        #vis fn #wrapper_name #generics(
            slf: &::rustpython_vm::Py<#self_ty>,
            vm: &::rustpython_vm::VirtualMachine,
        ) -> ::rustpython_vm::PyResult<::rustpython_vm::PyObjectRef> {
            let _vm = vm;
            let __py = ::pyo3::Python::from_vm(_vm);
            let slf = slf.to_owned();
            let mut slf = ::pyo3::PyRefMut::from_vm_ref(__py, slf);
            let __result = #call_expr;
            ::pyo3::__next_option_to_result(__result, __py)
        }

        #[allow(dead_code)]
        #method
    }
}

// ─── Regular Method Wrappers ──────────────────────────────────────────────────

fn needs_wrapper(method: &ImplItemFn) -> bool {
    if method.sig.inputs.iter().any(|arg| matches!(arg, FnArg::Receiver(r) if r.mutability.is_some())) {
        return true;
    }
    if returns_pyo3_result(&method.sig.output) {
        return true;
    }
    if has_pyo3_params(method) {
        return true;
    }
    let ret_str = return_type_string(&method.sig.output);
    if ret_str.contains("Py<")
        || ret_str.contains("Bound<")
        || ret_str.starts_with("Option<")
        || ret_str.starts_with("Vec<")
        || ret_str.starts_with("(")
    {
        return true;
    }
    false
}

fn generate_pyresult_wrapper(method: &ImplItemFn, self_ty: &syn::Type) -> TokenStream {
    let fn_name = &method.sig.ident;
    let wrapper_name = format_ident!("__pyo3_wrap_{}", fn_name);
    let helper_name = format_ident!("_pyo3_{}", fn_name);
    let vis = syn::Visibility::Inherited;
    let attrs = &method.attrs;
    let generics = &method.sig.generics;
    let fn_name_str = fn_name.to_string();

    let is_result = returns_pyo3_result(&method.sig.output);
    let ret_str = return_type_string(&method.sig.output);

    let mut wrapper_params: Vec<TokenStream> = Vec::new();
    let mut inner_call_args: Vec<TokenStream> = Vec::new();
    let mut conversion_stmts: Vec<TokenStream> = Vec::new();
    let mut has_py_param = false;

    for arg in &method.sig.inputs {
        match arg {
            FnArg::Receiver(_) => continue,
            FnArg::Typed(pt) => {
                let ty = &pt.ty;
                let ty_str = quote!(#ty).to_string().replace(' ', "");
                let name = if let Pat::Ident(pi) = pt.pat.as_ref() {
                    pi.ident.clone()
                } else {
                    wrapper_params.push(quote! { #arg });
                    inner_call_args.push(quote! { #arg });
                    continue;
                };

                let ty = &pt.ty;
                let raw_name = format_ident!("__raw_{}", name);

                if ty_str.contains("Python") {
                    has_py_param = true;
                    inner_call_args.push(quote! { __py });
                } else if ty_str.contains("VirtualMachine") {
                    inner_call_args.push(quote! { _vm });
                } else if ty_str == "&Self" || ty_str.ends_with("::Self") {
                    wrapper_params.push(quote! { #raw_name: ::rustpython_vm::PyObjectRef });
                    conversion_stmts.push(quote! {
                        let __bound_self = ::pyo3::Bound::from_object(__py, #raw_name);
                        let #name: &#self_ty = match <&#self_ty as ::pyo3::FromPyObject>::extract_bound(&__bound_self) {
                            Ok(v) => v,
                            Err(e) => return Err(::pyo3::err::into_vm_err(e)),
                        };
                    });
                    inner_call_args.push(quote! { #name });
                } else if ty_str.contains("Bound<") || ty_str.contains("&Bound<") {
                    let (extraction, call_expr) =
                        gen_bound_extraction(&ty_str, &name, &raw_name);
                    wrapper_params.push(quote! { #raw_name: ::rustpython_vm::PyObjectRef });
                    conversion_stmts.push(extraction);
                    inner_call_args.push(call_expr);
                } else if ty_str.contains("PyRefMut<") {
                    wrapper_params.push(quote! { __slf: ::rustpython_vm::PyRef<Self> });
                    conversion_stmts.push(quote! {
                        let #name = unsafe { ::rustpython_vm::PyRefMut::from_pyref_unchecked(__slf) };
                    });
                    inner_call_args.push(quote! { #name });
                } else if ty_str.contains("PyRef<") {
                    wrapper_params.push(quote! { #name: ::rustpython_vm::PyRef<Self> });
                    inner_call_args.push(quote! { #name });
                } else if ty_str.starts_with("Option<&") {
                    let (extraction, call_expr) = gen_option_bound_extraction(&ty_str, &name);
                    wrapper_params.push(quote! { #name: ::rustpython_vm::PyObjectRef });
                    conversion_stmts.push(extraction);
                    inner_call_args.push(call_expr);
                } else if ty_str.starts_with("Option<")
                    && !ty_str.contains("Bound")
                    && !ty_str.contains("Py<")
                {
                    let raw_name = format_ident!("__raw_{}", name);
                    let (extraction, call_expr) = gen_option_extraction(&ty_str, &name, &raw_name);
                    wrapper_params.push(quote! { #raw_name: ::rustpython_vm::PyObjectRef });
                    conversion_stmts.push(extraction);
                    inner_call_args.push(call_expr);
                } else {
                    if ty_str.contains("PathBuf") {
                        wrapper_params.push(quote! { #raw_name: ::rustpython_vm::PyObjectRef });
                        conversion_stmts.push(quote! {
                            let #name: ::std::path::PathBuf = {
                                let __s: ::rustpython_vm::builtins::PyStrRef = match <::rustpython_vm::builtins::PyStrRef as ::rustpython_vm::convert::TryFromObject>::try_from_object(
                                _vm,
                                #raw_name,
                            ) {
                                    Ok(v) => v,
                                    Err(e) => return Err(e),
                                };
                                ::std::path::PathBuf::from(__s.to_string())
                            };
                        });
                    } else {
                        let t: TokenStream = ty_str.parse().unwrap();
                        wrapper_params.push(quote! { #raw_name: ::rustpython_vm::PyObjectRef });
                        conversion_stmts.push(quote! {
                            let #name: #t = match <#t as ::pyo3::FromPyObject>::extract_bound(
                                &::pyo3::Bound::from_object(__py, #raw_name.clone())
                            ) {
                                Ok(v) => v,
                                Err(e) => return Err(::pyo3::err::into_vm_err(e)),
                            };
                        });
                    }
                    inner_call_args.push(quote! { #name });
                }
            }
        }
    }

    let inner_params: Vec<_> = method
        .sig
        .inputs
        .iter()
        .filter(|a| !matches!(a, FnArg::Receiver(_)))
        .collect();
    let helper_call_args: Vec<TokenStream> = method
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            FnArg::Receiver(_) => None,
            FnArg::Typed(pt) => match pt.pat.as_ref() {
                Pat::Ident(pi) => Some(quote! { #pi }),
                _ => None,
            },
        })
        .collect();
    let inner_ret = &method.sig.output;
    let inner_body = &method.block;
    let has_receiver = method
        .sig
        .inputs
        .iter()
        .any(|a| matches!(a, FnArg::Receiver(_)));

    let wrapper_receiver = method.sig.inputs.first().and_then(|a| {
        if let FnArg::Receiver(r) = a {
            if r.mutability.is_some() {
                Some(quote! { __slf: &::rustpython_vm::Py<Self> })
            } else {
                Some(quote! { #r })
            }
        } else {
            None
        }
    });
    let wrapper_receiver = wrapper_receiver.unwrap_or(quote! { &self });
    let inner_receiver = method.sig.inputs.first().and_then(|a| {
        if let FnArg::Receiver(r) = a {
            Some(quote! { #r })
        } else {
            None
        }
    });
    let inner_receiver = inner_receiver.unwrap_or(quote! { &self });
    let inner_signature = if has_receiver {
        quote! { (#inner_receiver, #(#inner_params),*) }
    } else {
        quote! { (#(#inner_params),*) }
    };
    let mut_receiver = method
        .sig
        .inputs
        .first()
        .and_then(|a| match a {
            FnArg::Receiver(r) => Some(r.mutability.is_some()),
            _ => None,
        })
        .unwrap_or(false);
    let call_expr = if has_receiver {
        if mut_receiver {
            quote! {
                {
                    let mut self_ = ::pyo3::PyRefMut::from_vm_ref(__py, __slf.to_owned());
                    self_.#fn_name(#(#inner_call_args),*)
                }
            }
        } else {
            quote! { self.#fn_name(#(#inner_call_args),*) }
        }
    } else {
        quote! { Self::#fn_name(#(#inner_call_args),*) }
    };
    let helper_call_expr = if has_receiver {
        quote! { self.#fn_name(#(#helper_call_args),*) }
    } else {
        quote! { Self::#fn_name(#(#helper_call_args),*) }
    };

    let py_inject = quote! {};

    if is_result {
        let ret_handling = convert_result_return_to_pyobj(&method.sig.output);
        quote! {
            #(#attrs)*
            #[pymethod(name = #fn_name_str)]
            #vis fn #wrapper_name #generics(#wrapper_receiver, #(#wrapper_params,)* vm: &::rustpython_vm::VirtualMachine) -> ::rustpython_vm::PyResult<::rustpython_vm::PyObjectRef> {
                let _vm = vm;
                let __py = ::pyo3::Python::from_vm(_vm);
                let py = __py;
                let vm = _vm;
                #py_inject
                #(#conversion_stmts)*
                let __result = #call_expr;
                #ret_handling
            }

            #[allow(dead_code)]
            #vis fn #fn_name #generics #inner_signature #inner_ret #inner_body

            #[allow(dead_code)]
            #vis fn #helper_name #generics #inner_signature #inner_ret {
                #helper_call_expr
            }
        }
    } else {
        let ret_handling = convert_non_result_return_to_pyobj(&method.sig.output);
        quote! {
            #(#attrs)*
            #[pymethod(name = #fn_name_str)]
            #vis fn #wrapper_name #generics(#wrapper_receiver, #(#wrapper_params,)* vm: &::rustpython_vm::VirtualMachine) -> ::rustpython_vm::PyResult<::rustpython_vm::PyObjectRef> {
                let _vm = vm;
                let __py = ::pyo3::Python::from_vm(_vm);
                let py = __py;
                let vm = _vm;
                #py_inject
                #(#conversion_stmts)*
                let __result = #call_expr;
                #ret_handling
            }

            #[allow(dead_code)]
            #vis fn #fn_name #generics #inner_signature #inner_ret #inner_body

            #[allow(dead_code)]
            #vis fn #helper_name #generics #inner_signature #inner_ret {
                #helper_call_expr
            }
        }
    }
}

fn generate_staticmethod_wrapper(method: &ImplItemFn, self_ty: &syn::Type) -> Result<TokenStream> {
    let fn_name = &method.sig.ident;
    let wrapper_name = format_ident!("_pyo3_static_{}", fn_name);
    let vis = syn::Visibility::Inherited;
    let generics = &method.sig.generics;
    let fn_name_str = fn_name.to_string();

    let (extraction, call_args) = generate_funcargs_extraction(method)?;
    let inner_body = &method.block;
    let inner_ret = &method.sig.output;
    let inner_params: Vec<_> = method
        .sig
        .inputs
        .iter()
        .filter(|a| !matches!(a, FnArg::Receiver(_)))
        .collect();

    let is_result = returns_pyo3_result(&method.sig.output);
    let ret_handling = if is_result {
        convert_result_return_to_pyobj(&method.sig.output)
    } else {
        convert_non_result_return_to_pyobj(&method.sig.output)
    };

    Ok(quote! {
        #[pystaticmethod(name = #fn_name_str)]
        #vis fn #wrapper_name #generics(__args: ::rustpython_vm::function::FuncArgs, vm: &::rustpython_vm::VirtualMachine) -> ::rustpython_vm::PyResult<::rustpython_vm::PyObjectRef> {
            let _vm = vm;
            let mut __args = __args;
            let __py = ::pyo3::Python::from_vm(_vm);
            let py = __py;
            let vm = _vm;
            #extraction
            let __result = Self::#fn_name(#call_args);
            #ret_handling
        }

        #[allow(dead_code)]
        #vis fn #fn_name #generics(#(#inner_params),*) #inner_ret #inner_body
    })
}

fn generate_classmethod_wrapper(method: &ImplItemFn, self_ty: &syn::Type) -> Result<TokenStream> {
    let fn_name = &method.sig.ident;
    let wrapper_name = format_ident!("_pyo3_classmethod_{}", fn_name);
    let vis = syn::Visibility::Inherited;
    let generics = &method.sig.generics;
    let fn_name_str = fn_name.to_string();

    let (extraction, call_args) = generate_funcargs_extraction(method)?;
    let inner_body = &method.block;
    let inner_ret = &method.sig.output;
    let inner_params: Vec<_> = method
        .sig
        .inputs
        .iter()
        .filter(|a| !matches!(a, FnArg::Receiver(_)))
        .collect();

    let is_result = returns_pyo3_result(&method.sig.output);
    let ret_handling = if is_result {
        convert_result_return_to_pyobj(&method.sig.output)
    } else {
        convert_non_result_return_to_pyobj(&method.sig.output)
    };

    Ok(quote! {
        #[pyclassmethod(name = #fn_name_str)]
        #vis fn #wrapper_name #generics(__cls: &::rustpython_vm::Py<::rustpython_vm::builtins::PyType>, __args: ::rustpython_vm::function::FuncArgs, vm: &::rustpython_vm::VirtualMachine) -> ::rustpython_vm::PyResult<::rustpython_vm::PyObjectRef> {
            let _vm = vm;
            let mut __args = __args;
            let __py = ::pyo3::Python::from_vm(_vm);
            let py = __py;
            let vm = _vm;
            #extraction
            let __result = Self::#fn_name(#call_args);
            #ret_handling
        }

        #[allow(dead_code)]
        #vis fn #fn_name #generics(#(#inner_params),*) #inner_ret #inner_body
    })
}

// ─── FuncArgs-based Extraction (shared with pyfunction) ───────────────────────

fn generate_funcargs_extraction(func: &ImplItemFn) -> Result<(TokenStream, TokenStream)> {
    let mut extraction_stmts: Vec<TokenStream> = Vec::new();
    let mut call_exprs: Vec<TokenStream> = Vec::new();

    for arg in &func.sig.inputs {
        let FnArg::Typed(pt) = arg else { continue };
        let pat_name = match pt.pat.as_ref() {
            Pat::Ident(pi) => pi.ident.clone(),
            _ => continue,
        };

        let ty = &pt.ty;
        let ty_str = quote!(#ty).to_string().replace(' ', "");

        if ty_str.contains("Python") {
            call_exprs.push(quote! { ::pyo3::Python::from_vm(_vm) });
            continue;
        }

        let (extraction, call_expr) = gen_extraction_for_ty(&ty_str, &pat_name)?;
        extraction_stmts.push(extraction);
        call_exprs.push(call_expr);
    }

    let all_extraction = quote! { #(#extraction_stmts)* };
    let all_calls = quote! { #(#call_exprs),* };

    Ok((all_extraction, all_calls))
}

fn gen_extraction_for_ty(ty_str: &str, name: &syn::Ident) -> Result<(TokenStream, TokenStream)> {
    if ty_str.contains("Python") {
        return Ok((quote! {}, quote! { ::pyo3::Python::from_vm(_vm) }));
    }

    if let Some(inner) = ty_str.strip_prefix("&") {
        if inner.contains("Bound") {
            let bound_name = format_ident!("__bound_{}", name);
            return Ok((
                quote! {
                    let #name = __args.take_positional().ok_or_else(|| {
                        _vm.new_type_error(format!("missing required argument: {}", stringify!(#name)))
                    })?;
                    let #bound_name = ::pyo3::Bound::from_object(::pyo3::Python::from_vm(_vm), #name);
                },
                quote! { &#bound_name },
            ));
        }
        if inner.contains("Py<") {
            let inner_t: TokenStream = inner.parse().unwrap();
            return Ok((
                quote! {
                    let #name: #inner_t = ::pyo3::Py::from(
                        __args.take_positional().ok_or_else(|| {
                            _vm.new_type_error(format!("missing required argument: {}", stringify!(#name)))
                        })?
                    );
                },
                quote! { #name },
            ));
        }
    }

    if let Some(inner) = ty_str
        .strip_prefix("Option<")
        .and_then(|s| s.strip_suffix('>'))
    {
        return gen_option_extraction_for_ty(inner, name);
    }

    match &*ty_str {
        "&str" => Ok((
            quote! {
                let #name: ::rustpython_vm::builtins::PyStrRef = __args.take_positional().ok_or_else(|| {
                    _vm.new_type_error(format!("missing required argument: {}", stringify!(#name)))
                })?.try_into_value(_vm).map_err(|e| e)?;
            },
            quote! { #name.as_str() },
        )),
        "String" => Ok((
            quote! {
                let #name: ::rustpython_vm::builtins::PyStrRef = __args.take_positional().ok_or_else(|| {
                    _vm.new_type_error(format!("missing required argument: {}", stringify!(#name)))
                })?.try_into_value(_vm).map_err(|e| e)?;
            },
            quote! { #name.to_string() },
        )),
        "bool" => Ok((
            quote! {
                let #name: bool = __args.take_positional().ok_or_else(|| {
                    _vm.new_type_error(format!("missing required argument: {}", stringify!(#name)))
                })?.try_into_value(_vm).map_err(|e| e)?;
            },
            quote! { #name },
        )),
        "i8" | "i16" | "i32" | "i64" => {
            let t: TokenStream = ty_str.parse().unwrap();
            Ok((
                quote! {
                    let #name: i64 = __args.take_positional().ok_or_else(|| {
                        _vm.new_type_error(format!("missing required argument: {}", stringify!(#name)))
                    })?.try_into_value(_vm).map_err(|e| e)?;
                },
                quote! { #name as #t },
            ))
        }
        "u8" | "u16" | "u32" | "u64" | "usize" => {
            let t: TokenStream = ty_str.parse().unwrap();
            Ok((
                quote! {
                    let #name: u64 = __args.take_positional().ok_or_else(|| {
                        _vm.new_type_error(format!("missing required argument: {}", stringify!(#name)))
                    })?.try_into_value(_vm).map_err(|e| e)?;
                },
                quote! { #name as #t },
            ))
        }
        "f32" | "f64" => {
            let t: TokenStream = ty_str.parse().unwrap();
            Ok((
                quote! {
                    let #name: f64 = __args.take_positional().ok_or_else(|| {
                        _vm.new_type_error(format!("missing required argument: {}", stringify!(#name)))
                    })?.try_into_value(_vm).map_err(|e| e)?;
                },
                quote! { #name as #t },
            ))
        }
        _ => {
            if ty_str.contains("Bound") {
                let bound_name = format_ident!("__bound_{}", name);
                Ok((
                    quote! {
                        let #name = __args.take_positional().ok_or_else(|| {
                            _vm.new_type_error(format!("missing required argument: {}", stringify!(#name)))
                        })?;
                        let #bound_name = ::pyo3::Bound::from_object(::pyo3::Python::from_vm(_vm), #name);
                    },
                    quote! { #bound_name },
                ))
            } else {
                let t: TokenStream = ty_str.parse().unwrap();
                Ok((
                    quote! {
                        let #name = match <#t as ::pyo3::FromPyObject>::extract_bound(
                            &::pyo3::Bound::from_object(
                                ::pyo3::Python::from_vm(_vm),
                                __args.take_positional().ok_or_else(|| {
                                    _vm.new_type_error(format!("missing required argument: {}", stringify!(#name)))
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
}

fn gen_option_extraction_for_ty(
    inner: &str,
    name: &syn::Ident,
) -> Result<(TokenStream, TokenStream)> {
    if let Some(bound_inner) = inner.strip_prefix("&") {
        if bound_inner.contains("Bound") {
            let bound_name = format_ident!("__bound_{}", name);
            return Ok((
                quote! {
                    let #name = __args.take_positional();
                    let #bound_name = match #name {
                        Some(__obj) => {
                            if _vm.is_none(&__obj) { None }
                            else { Some(::pyo3::Bound::from_object(::pyo3::Python::from_vm(_vm), __obj)) }
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
                    let #name = __args.take_positional();
                    let #name: Option<#inner_t> = match #name {
                        Some(__obj) => {
                            if _vm.is_none(&__obj) { None }
                            else { Some(::pyo3::Py::from(__obj)) }
                        }
                        None => None,
                    };
                },
                quote! { #name.as_ref() },
            ));
        }
    }

    match inner {
        "bool" => Ok((
            quote! {
                let #name = __args.take_positional();
                let #name: Option<bool> = match #name {
                    Some(__obj) => {
                        if _vm.is_none(&__obj) { None }
                        else { Some(__obj.try_into_value(_vm).map_err(|e| e)?) }
                    }
                    None => None,
                };
            },
            quote! { #name },
        )),
        "u8" | "u16" | "u32" => {
            let t: TokenStream = inner.parse().unwrap();
            Ok((
                quote! {
                    let #name = __args.take_positional();
                    let #name: Option<#t> = match #name {
                        Some(__obj) => {
                            if _vm.is_none(&__obj) { None }
                            else { Some(__obj.try_into_value(_vm).map_err(|e| e)?) }
                        }
                        None => None,
                    };
                },
                quote! { #name },
            ))
        }
        "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64" => {
            let t: TokenStream = inner.parse().unwrap();
            Ok((
                quote! {
                    let #name = __args.take_positional();
                    let #name: Option<#t> = match #name {
                        Some(__obj) => {
                            if _vm.is_none(&__obj) { None }
                            else { Some(__obj.try_into_value(_vm).map_err(|e| e)?) }
                        }
                        None => None,
                    };
                },
                quote! { #name },
            ))
        }
        "String" => Ok((
            quote! {
                let #name = __args.take_positional();
                let #name: Option<String> = match #name {
                    Some(__obj) => {
                        if _vm.is_none(&__obj) { None }
                        else {
                            let __s: ::rustpython_vm::builtins::PyStrRef = __obj.try_into_value(_vm).map_err(|e| e)?;
                            Some(__s.to_string())
                        }
                    }
                    None => None,
                };
            },
            quote! { #name },
        )),
        _ => {
            if inner.contains("Bound") {
                let bound_name = format_ident!("__bound_{}", name);
                Ok((
                    quote! {
                        let #name = __args.take_positional();
                        let #bound_name = match #name {
                            Some(__obj) => {
                                if _vm.is_none(&__obj) { None }
                                else { Some(::pyo3::Bound::from_object(::pyo3::Python::from_vm(_vm), __obj)) }
                            }
                            None => None,
                        };
                    },
                    quote! { #bound_name },
                ))
            } else {
                let t: TokenStream = inner.parse().unwrap();
                Ok((
                    quote! {
                        let #name = __args.take_positional();
                        let #name: Option<#t> = match #name {
                            Some(__obj) => {
                                if _vm.is_none(&__obj) { None }
                                else {
                                    match <#t as ::pyo3::FromPyObject>::extract_bound(
                                &::pyo3::Bound::from_object(::pyo3::Python::from_vm(_vm), __obj)
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
}

// ─── Bound/Option Extraction Helpers for Wrappers ─────────────────────────────

fn gen_bound_extraction(
    ty_str: &str,
    name: &syn::Ident,
    wrapper_name: &syn::Ident,
) -> (TokenStream, TokenStream) {
    if ty_str.contains("&Bound") {
        (
            quote! {
                let #name = ::pyo3::Bound::from_object(__py, #wrapper_name);
            },
            quote! { &#name },
        )
    } else {
        (
            quote! {
                let #name = ::pyo3::Bound::from_object(__py, #wrapper_name);
            },
            quote! { #name },
        )
    }
}

fn gen_option_bound_extraction(ty_str: &str, name: &syn::Ident) -> (TokenStream, TokenStream) {
    let inner = ty_str
        .strip_prefix("Option<")
        .and_then(|s| s.strip_suffix('>'))
        .unwrap_or(ty_str);
    let raw_name = format_ident!("__raw_{}", name);
    if inner.contains("Bound") {
        let call_expr = if inner.contains("&Bound") {
            quote! { #name.as_ref() }
        } else {
            quote! { #name }
        };
        (
            quote! {
                let #name = if _vm.is_none(&#raw_name) { None } else { Some(::pyo3::Bound::from_object(__py, #raw_name)) };
            },
            call_expr,
        )
    } else {
        (
            quote! {
                let #name = if _vm.is_none(&#raw_name) { None } else { Some(::pyo3::Py::from(#raw_name)) };
            },
            quote! { #name },
        )
    }
}

fn gen_option_extraction(
    ty_str: &str,
    name: &syn::Ident,
    raw_name: &syn::Ident,
) -> (TokenStream, TokenStream) {
    let inner = ty_str
        .strip_prefix("Option<")
        .and_then(|s| s.strip_suffix('>'))
        .unwrap_or(ty_str);
    match inner {
        "bool" => (
            quote! {
                let #name: Option<bool> = if _vm.is_none(&#raw_name) { None } else { #raw_name.try_into_value(_vm).ok() };
            },
            quote! { #name },
        ),
        "u8" | "u16" | "u32" | "u64" | "usize" | "i8" | "i16" | "i32" | "i64" | "f32" | "f64" => {
            let t: TokenStream = inner.parse().unwrap();
            (
                quote! {
                    let #name: Option<#t> = if _vm.is_none(&#raw_name) { None } else { #raw_name.try_into_value(_vm).ok() };
                },
                quote! { #name },
            )
        }
        "String" => (
            quote! {
                    let #name: Option<String> = if _vm.is_none(&#raw_name) { None } else {
                        let __s: ::rustpython_vm::builtins::PyStrRef = #raw_name.try_into_value(_vm).map_err(|e| e)?;
                    Some(__s.to_string())
                };
            },
            quote! { #name },
        ),
        _ => {
            let t: TokenStream = inner.parse().unwrap();
            (
                quote! {
                        let #name: Option<#t> = if _vm.is_none(&#raw_name) { None } else {
                            match <#t as ::pyo3::FromPyObject>::extract_bound(
                                &::pyo3::Bound::from_object(::pyo3::Python::from_vm(_vm), #raw_name.clone())
                            ) {
                            Ok(v) => Some(v),
                            Err(e) => return Err(::pyo3::err::into_vm_err(e)),
                        }
                    };
                },
                quote! { #name },
            )
        }
    }
}

fn generate_slot_param_extraction(method: &ImplItemFn) -> (TokenStream, TokenStream) {
    let mut extraction = Vec::new();
    let mut call_args = Vec::new();

    for arg in &method.sig.inputs {
        if let FnArg::Typed(pt) = arg {
            let ty = &pt.ty;
            let ty_str = quote!(#ty).to_string().replace(' ', "");
            if ty_str.contains("Python") {
                call_args.push(quote! { ::pyo3::Python::from_vm(_vm) });
                continue;
            }
            if ty_str.contains("&Bound") {
                let name = if let Pat::Ident(pi) = pt.pat.as_ref() {
                    pi.ident.clone()
                } else {
                    continue;
                };
                let raw_name = format_ident!("__raw_{}", name);
                extraction.push(quote! {
                    let #raw_name: ::rustpython_vm::PyObjectRef = _vm.ctx.none();
                    let #name = ::pyo3::Bound::from_object(::pyo3::Python::from_vm(_vm), #raw_name);
                });
                call_args.push(quote! { &#name });
            } else {
                if let Pat::Ident(pi) = pt.pat.as_ref() {
                    call_args.push(quote! { #pi });
                }
            }
        }
    }

    (quote! { #(#extraction)* }, quote! { #(#call_args),* })
}

// ─── Return Type Conversion ──────────────────────────────────────────────────

fn convert_result_return_to_pyobj(ret: &ReturnType) -> TokenStream {
    let ret_str = return_type_string(ret);
    let inner = extract_pyresult_inner(ret);
    let is_rustpython_result = ret_str.contains("rustpython_vm::PyResult");

    match inner.as_ref().map(|s| s.to_string()).as_deref() {
        Some("()") => {
            if is_rustpython_result {
                quote! { __result.map(|_| _vm.ctx.none()) }
            } else {
                quote! { __result.map(|_| _vm.ctx.none()).map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e)) }
            }
        }
        Some(t) if t.starts_with("Option<") => {
            if is_rustpython_result {
                quote! {
                    __result.and_then(|v| {
                        ::pyo3::IntoPyObject::into_pyobject(v, __py)
                            .map(|b| b.into_any().unbind().into_object())
                            .map_err(|e| ::pyo3::err::into_vm_err(e.into()))
                    })
                }
            } else {
                quote! {
                    __result.and_then(|v| {
                        ::pyo3::IntoPyObject::into_pyobject(v, __py)
                            .map(|b| b.into_any().unbind().into_object())
                    }).map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e))
                }
            }
        }
        Some(t) if t.starts_with("(") => {
            if is_rustpython_result {
                quote! {
                    __result.and_then(|v| {
                        ::pyo3::IntoPyObject::into_pyobject(v, __py)
                            .map(|b| b.into_any().unbind().into_object())
                            .map_err(|e| ::pyo3::err::into_vm_err(e.into()))
                    })
                }
            } else {
                quote! {
                    __result.and_then(|v| {
                        ::pyo3::IntoPyObject::into_pyobject(v, __py)
                            .map(|b| b.into_any().unbind().into_object())
                    }).map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e))
                }
            }
        }
        Some("bool") => {
            if is_rustpython_result {
                quote! { __result.map(|v| ::rustpython_vm::convert::ToPyObject::to_pyobject(v, _vm)) }
            } else {
                quote! { __result.map(|v| ::rustpython_vm::convert::ToPyObject::to_pyobject(v, _vm)).map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e)) }
            }
        }
        Some(t) if t.contains("Py<") => {
            if is_rustpython_result {
                quote! { __result.map(|v| ::rustpython_vm::convert::IntoObject::into_object(v)) }
            } else {
                quote! { __result.map(|v| ::rustpython_vm::convert::IntoObject::into_object(v)).map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e)) }
            }
        }
        Some(t) if t.contains("Bound<") => {
            if is_rustpython_result {
                quote! { __result.map(|v| ::rustpython_vm::convert::IntoObject::into_object(v)) }
            } else {
                quote! { __result.map(|v| ::rustpython_vm::convert::IntoObject::into_object(v)).map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e)) }
            }
        }
        Some(t) if t.starts_with("&str") => {
            if is_rustpython_result {
                quote! { __result.map(|v| ::rustpython_vm::convert::ToPyObject::to_pyobject(v.to_string(), _vm)) }
            } else {
                quote! { __result.map(|v| ::rustpython_vm::convert::ToPyObject::to_pyobject(v.to_string(), _vm)).map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e)) }
            }
        }
        Some(t) if t == "String" || t == "&str" => {
            if is_rustpython_result {
                quote! { __result.map(|v| ::rustpython_vm::convert::ToPyObject::to_pyobject(v, _vm)) }
            } else {
                quote! { __result.map(|v| ::rustpython_vm::convert::ToPyObject::to_pyobject(v, _vm)).map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e)) }
            }
        }
        Some(_) => {
            if is_rustpython_result {
                quote! { __result.map(|v| ::rustpython_vm::convert::ToPyObject::to_pyobject(v, _vm)) }
            } else {
                quote! { __result.map(|v| ::rustpython_vm::convert::ToPyObject::to_pyobject(v, _vm)).map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e)) }
            }
        }
        None => {
            if is_rustpython_result {
                quote! { __result }
            } else {
                quote! { __result.map_err(|e: ::pyo3::PyErr| ::pyo3::err::into_vm_err(e)) }
            }
        }
    }
}

fn convert_non_result_return_to_pyobj(ret: &ReturnType) -> TokenStream {
    let ret_str = return_type_string(ret);

    match ret_str.as_str() {
        "()" => quote! { Ok(_vm.ctx.none()) },
        s if s.starts_with("Option<") => {
            quote! {
                ::pyo3::IntoPyObject::into_pyobject(__result, __py)
                    .map(|b| b.into_any().unbind().into_object())
                    .map_err(|e| ::pyo3::err::into_vm_err(e.into()))
            }
        }
        s if s.starts_with("(") => {
            quote! {
                ::pyo3::IntoPyObject::into_pyobject(__result, __py)
                    .map(|b| b.into_any().unbind().into_object())
                    .map_err(|e| ::pyo3::err::into_vm_err(e.into()))
            }
        }
        "bool" | "i8" | "i16" | "i32" | "i64" | "u8" | "u16" | "u32" | "u64" | "usize" | "f32"
        | "f64" | "String" => {
            quote! { Ok(::rustpython_vm::convert::ToPyObject::to_pyobject(__result, _vm)) }
        }
        s if s.contains("Py<") => quote! { Ok(::rustpython_vm::convert::IntoObject::into_object(__result)) },
        s if s.contains("Bound<") => quote! { Ok(::rustpython_vm::convert::IntoObject::into_object(__result)) },
        s if s.starts_with("&str") => {
            quote! { Ok(::rustpython_vm::convert::ToPyObject::to_pyobject(__result.to_string(), _vm)) }
        }
        _ => quote! {
            ::pyo3::IntoPyObject::into_pyobject(__result, __py)
                .map(|b| b.into_any().unbind().into_object())
                .map_err(|e| ::pyo3::err::into_vm_err(e.into()))
        },
    }
}

fn convert_return_to_pyobj(ret: &ReturnType, is_result: bool) -> TokenStream {
    if is_result {
        convert_result_return_to_pyobj(ret)
    } else {
        convert_non_result_return_to_pyobj(ret)
    }
}

fn extract_option_inner(ret: &ReturnType) -> TokenStream {
    match ret {
        ReturnType::Default => quote! { () },
        ReturnType::Type(_, ty) => {
            let s = quote!(#ty).to_string().replace(' ', "");
            if let Some(inner) = s.strip_prefix("Option<").and_then(|s| s.strip_suffix('>')) {
                inner.parse().unwrap_or(quote! { () })
            } else {
                quote! { () }
            }
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn is_rustpython_slot_method(name: &str) -> bool {
    matches!(
        name,
        "__hash__"
            | "__eq__"
            | "__ne__"
            | "__lt__"
            | "__le__"
            | "__gt__"
            | "__ge__"
            | "__richcmp__"
            | "__and__"
            | "__or__"
            | "__sub__"
            | "__xor__"
            | "__add__"
            | "__mul__"
            | "__truediv__"
            | "__floordiv__"
            | "__mod__"
            | "__pow__"
            | "__lshift__"
            | "__rshift__"
            | "__matmul__"
            | "__neg__"
            | "__pos__"
            | "__abs__"
            | "__invert__"
            | "__int__"
            | "__float__"
            | "__bool__"
            | "__iadd__"
            | "__isub__"
            | "__imul__"
            | "__iand__"
            | "__ior__"
            | "__ixor__"
            | "__concat__"
            | "__inplace_concat__"
            | "__repeat__"
            | "__inplace_repeat__"
            | "__contains__"
            | "__iter__"
            | "__next__"
            | "__len__"
            | "__getitem__"
            | "__setitem__"
            | "__delitem__"
            | "__reversed__"
            | "__reduce__"
            | "__repr__"
            | "__str__"
            | "__call__"
    )
}

fn has_attr(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

fn strip_pyo3_method_attrs(method: &ImplItemFn) -> ImplItemFn {
    let pyo3_attrs = [
        "new",
        "getter",
        "setter",
        "staticmethod",
        "classmethod",
        "classattr",
        "pyo3",
    ];
    let mut cleaned = method.clone();
    cleaned
        .attrs
        .retain(|attr| !pyo3_attrs.iter().any(|name| attr.path().is_ident(name)));
    cleaned
}

fn sanitize_typed_patterns(method: &mut ImplItemFn) {
    for arg in &mut method.sig.inputs {
        if let FnArg::Typed(pt) = arg {
            if let Pat::Ident(pi) = pt.pat.as_mut() {
                pi.mutability = None;
                pi.by_ref = None;
            }
        }
    }
}

fn strip_pyo3_const_attrs(item: &ImplItemConst) -> ImplItemConst {
    let pyo3_attrs = [
        "new",
        "getter",
        "setter",
        "staticmethod",
        "classmethod",
        "pyo3",
        "classattr",
    ];
    let mut cleaned = item.clone();
    cleaned
        .attrs
        .retain(|attr| !pyo3_attrs.iter().any(|name| attr.path().is_ident(name)));
    cleaned
}

fn has_pyo3_params(method: &ImplItemFn) -> bool {
    for arg in &method.sig.inputs {
        if let FnArg::Typed(pt) = arg {
            let param_str = quote!(#pt).to_string();
            if param_str.contains("Python")
                || param_str.contains("Bound")
                || param_str.contains("Py <")
                || param_str.contains("Py<")
            {
                return true;
            }
            let ty = &pt.ty;
            let ty_str = quote!(#ty).to_string().replace(' ', "");
            if ty_str.contains("&Self") || ty_str == "Self" {
                return true;
            }
        }
    }
    false
}

fn has_vm_virtualmachine_param(method: &ImplItemFn) -> bool {
    method.sig.inputs.iter().any(|arg| {
        if let FnArg::Typed(pt) = arg {
            let ty = &pt.ty;
            let s = quote!(#ty).to_string().replace(' ', "");
            s.contains("VirtualMachine")
        } else {
            false
        }
    })
}

fn returns_pyo3_result(ret: &ReturnType) -> bool {
    match ret {
        ReturnType::Default => false,
        ReturnType::Type(_, ty) => {
            let s = quote!(#ty).to_string().replace(' ', "");
            s.starts_with("PyResult<") || s.contains("pyo3::PyResult<")
        }
    }
}

fn return_type_string(ret: &ReturnType) -> String {
    match ret {
        ReturnType::Default => String::new(),
        ReturnType::Type(_, ty) => quote!(#ty).to_string().replace(' ', ""),
    }
}

fn extract_pyresult_inner(ret: &ReturnType) -> Option<TokenStream> {
    match ret {
        ReturnType::Default => None,
        ReturnType::Type(_, ty) => {
            let s = quote!(#ty).to_string().replace(' ', "");
            if !s.contains("PyResult") {
                return None;
            }
            if let Some(start) = s.find('<') {
                if let Some(end) = s.rfind('>') {
                    let inner = &s[start + 1..end];
                    let inner_tokens: TokenStream = inner.parse().ok()?;
                    return Some(inner_tokens);
                }
            }
            None
        }
    }
}

fn collect_fn_params(func: &ImplItemFn) -> Result<Vec<(syn::Ident, syn::Type)>> {
    let mut params = Vec::new();
    for arg in &func.sig.inputs {
        match arg {
            FnArg::Receiver(_) => continue,
            FnArg::Typed(pat_type) => {
                let name = match pat_type.pat.as_ref() {
                    Pat::Ident(pi) => pi.ident.clone(),
                    other => {
                        return Err(syn::Error::new_spanned(
                            other,
                            "unsupported argument pattern in #[new]",
                        ))
                    }
                };
                params.push((name, (*pat_type.ty).clone()));
            }
        }
    }
    Ok(params)
}

fn parse_constructor_signature_defaults(attrs: &[syn::Attribute]) -> Result<Option<Vec<Option<Expr>>>> {
    for attr in attrs {
        if !attr.path().is_ident("pyo3") {
            continue;
        }

        let mut defaults = None;
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("signature") {
                return Ok(());
            }

            let value = meta.value()?;
            let content;
            syn::parenthesized!(content in value);

            let mut parsed = Vec::new();
            while !content.is_empty() {
                if content.peek(Token![/]) {
                    content.parse::<Token![/]>()?;
                } else if content.peek(Token![*]) {
                    content.parse::<Token![*]>()?;
                } else {
                    let _: syn::Ident = content.parse()?;
                    let default = if content.peek(Token![=]) {
                        content.parse::<Token![=]>()?;
                        Some(content.parse::<Expr>()?)
                    } else {
                        None
                    };
                    parsed.push(default);
                }

                if content.peek(Comma) {
                    content.parse::<Comma>()?;
                }
            }

            defaults = Some(parsed);
            Ok(())
        })?;

        if defaults.is_some() {
            return Ok(defaults);
        }
    }

    Ok(None)
}

fn is_none_expr(expr: &Expr) -> bool {
    matches!(
        expr,
        Expr::Path(path)
            if path.path.segments.last().is_some_and(|segment| segment.ident == "None")
    )
}
