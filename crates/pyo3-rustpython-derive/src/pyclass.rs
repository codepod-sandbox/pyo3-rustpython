use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{Fields, ItemEnum, ItemStruct, Result};

/// Information about a field annotated with `#[pyo3(get)]` and/or `#[pyo3(set)]`.
struct FieldAccessor {
    name: syn::Ident,
    ty: syn::Type,
    get: bool,
    set: bool,
}

/// Options parsed from `#[pyclass(name = "...", module = "...", frozen, set, get_all, from_py_object, subclass, extends=...)]`.
struct PyClassOptions {
    name: Option<String>,
    module: Option<String>,
    frozen: bool,
    set: bool,
    get_all: bool,
    from_py_object: bool,
    subclass: bool,
    extends: Option<syn::Path>,
    sequence: bool,
}

fn parse_pyclass_options(attr: &TokenStream) -> PyClassOptions {
    let mut opts = PyClassOptions {
        name: None,
        module: None,
        frozen: false,
        set: false,
        get_all: false,
        from_py_object: false,
        subclass: false,
        extends: None,
        sequence: false,
    };
    if attr.is_empty() {
        return opts;
    }
    let s = attr.to_string();
    for part in s.split(',') {
        let part = part.trim();
        if part == "frozen" {
            opts.frozen = true;
        } else if part == "set" {
            opts.set = true;
        } else if part == "get_all" {
            opts.get_all = true;
        } else if part == "sequence" {
            opts.sequence = true;
        } else if part == "from_py_object" {
            opts.from_py_object = true;
        } else if part == "subclass" {
            opts.subclass = true;
        } else if let Some(rest) = part.strip_prefix("name") {
            if let Some(val) = extract_string_value(rest) {
                opts.name = Some(val);
            }
        } else if let Some(rest) = part.strip_prefix("module") {
            if let Some(val) = extract_string_value(rest) {
                opts.module = Some(val);
            }
        } else if let Some(rest) = part.strip_prefix("extends") {
            let val = rest.trim().trim_start_matches('=').trim();
            if let Ok(path) = syn::parse_str::<syn::Path>(val) {
                opts.extends = Some(path);
            }
        }
    }
    opts
}

fn extract_string_value(s: &str) -> Option<String> {
    let s = s.trim();
    let s = s.strip_prefix('=')?;
    let s = s.trim();
    let s = s.strip_prefix('"')?;
    let s = s.strip_suffix('"')?;
    Some(s.to_string())
}

/// Expand `#[pyclass]` on a struct.
///
/// Generates:
/// 1. The struct with `#[rustpython_vm::pyclass(module = false, name = "...")]`
///    and `#[derive(rustpython_vm::PyPayload)]` added, `#[pyo3(...)]` stripped.
/// 2. A `Debug` derive if not already present.
/// 3. An impl of `pyo3::Pyo3Accessors` that registers getters/setters for
///    fields annotated with `#[pyo3(get)]` / `#[pyo3(set)]`.
pub fn expand(attr: TokenStream, mut input: ItemStruct) -> Result<TokenStream> {
    let options = parse_pyclass_options(&attr);

    let struct_name = &input.ident;
    let struct_name_str = options
        .name
        .clone()
        .unwrap_or_else(|| struct_name.to_string());

    let accessors = collect_accessors(&input.fields, options.get_all)?;

    strip_pyo3_attrs(&mut input.fields);

    let has_debug = input.attrs.iter().any(|attr| {
        if attr.path().is_ident("derive") {
            attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            )
            .map(|paths| {
                paths.iter().any(|p| {
                    p.is_ident("Debug") || p.segments.last().map_or(false, |s| s.ident == "Debug")
                })
            })
            .unwrap_or(false)
        } else {
            false
        }
    });

    let debug_impl = if has_debug {
        quote! {}
    } else {
        quote! {
            impl ::core::fmt::Debug for #struct_name {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.debug_struct(#struct_name_str).finish_non_exhaustive()
                }
            }
        }
    };

    let accessor_impl = generate_accessor_impl(struct_name, &accessors);
    let inventory_class_name = inventory_class_name(struct_name);
    let inventory_def = generate_inventory_class(&inventory_class_name);
    let pyclass_flags = pyclass_impl_flags(&options);

    let from_py_object_impl = if options.from_py_object {
        quote! {
            impl<'py> ::pyo3::FromPyObject<'py> for #struct_name {
                fn extract_bound(ob: &::pyo3::Bound<'py, ::pyo3::types::PyAny>) -> ::pyo3::PyResult<Self> {
                    let ref_val: &#struct_name = <&#struct_name as ::pyo3::FromPyObject<'py>>::extract_bound(ob)?;
                    Ok(ref_val.clone())
                }
            }
        }
    } else {
        quote! {}
    };

    let base_payload_impl = if let Some(ref base_path) = options.extends {
        quote! {
            impl ::pyo3::Pyo3BasePayload for #struct_name {
                type BasePayload = #base_path;
            }
        }
    } else {
        quote! {
            impl ::pyo3::Pyo3BasePayload for #struct_name {
                type BasePayload = ::rustpython_vm::builtins::PyBaseObject;
            }
        }
    };

    let base_init = if let Some(ref base_path) = options.extends {
        quote! {
            {
                let base_type = <#base_path as ::pyo3::PyTypeObjectExt>::type_object_raw(ctx);
                let child_type = <Self as ::rustpython_vm::class::StaticType>::static_type();
                *child_type.bases.write() = vec![base_type.to_owned()];
                let base_mro: Vec<_> = base_type.mro.read().iter().cloned().collect();
                let new_mro: Vec<_> = ::std::iter::once(child_type.to_owned())
                    .chain(base_mro)
                    .collect();
                *child_type.mro.write() = new_mro;
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        #[::rustpython_vm::pyclass(module = false, name = #struct_name_str)]
        #input

        #debug_impl

        impl ::rustpython_vm::PyPayload for #struct_name {
            #[inline]
            fn class(ctx: &::rustpython_vm::Context) -> &'static ::rustpython_vm::Py<::rustpython_vm::builtins::PyType> {
                let already_init = <Self as ::rustpython_vm::class::StaticType>::static_cell().get().is_some();
                let _ = ctx;
                <Self as ::rustpython_vm::class::PyClassImpl>::make_static_type();
                let typ = <Self as ::rustpython_vm::class::StaticType>::static_type();
                if !already_init {
                    #base_init
                    for __pyo3_items in ::pyo3::inventory::iter::<#inventory_class_name> {
                        let __pyo3_items = ::pyo3::Pyo3ClassInventory::items(__pyo3_items);
                        (__pyo3_items.extend_class)(ctx, typ);
                        typ.extend_methods(__pyo3_items.methods, ctx);
                        ::pyo3::slots::apply_inventory_slots(typ, __pyo3_items.extend_slots);
                    }
                    ::pyo3::slots::fixup_dunder_slots(typ, ctx);
                }
                typ
            }
        }

        impl<'py> ::pyo3::IntoPyObject<'py> for #struct_name {
            type Target = ::pyo3::types::PyAny;
            type Error = ::pyo3::PyErr;

            fn into_pyobject(self, py: ::pyo3::Python<'py>) -> Result<::pyo3::Bound<'py, Self::Target>, Self::Error> {
                let obj = ::rustpython_vm::convert::ToPyObject::to_pyobject(self, py.vm());
                Ok(::pyo3::Bound::from_object(py, obj))
            }
        }

        impl ::pyo3::PyTypeInfo for #struct_name {
            const NAME: &'static str = #struct_name_str;
            const MODULE: Option<&'static str> = None;
        }

        impl<'py> ::pyo3::FromPyObject<'py> for &'py #struct_name {
            fn extract_bound(ob: &::pyo3::Bound<'py, ::pyo3::types::PyAny>) -> ::pyo3::PyResult<Self> {
                ob.downcast_payload::<#struct_name>()
                    .ok_or_else(|| ::pyo3::PyErr::new_type_error(ob.py(), "type mismatch"))
            }
        }

        #from_py_object_impl
        #base_payload_impl

        #accessor_impl
        impl ::rustpython_vm::class::PyClassImpl for #struct_name {
            const TP_FLAGS: ::rustpython_vm::types::PyTypeFlags = #pyclass_flags;

            fn impl_extend_class(
                ctx: &'static ::rustpython_vm::Context,
                class: &'static ::rustpython_vm::Py<::rustpython_vm::builtins::PyType>,
            ) {
                <Self as ::pyo3::Pyo3Accessors>::__pyo3_register_accessors(ctx, class);
            }

            const METHOD_DEFS: &'static [::rustpython_vm::function::PyMethodDef] = &[];

            fn extend_slots(_slots: &mut ::rustpython_vm::types::PyTypeSlots) {}
        }
        #inventory_def
    })
}

fn collect_accessors(fields: &Fields, get_all: bool) -> Result<Vec<FieldAccessor>> {
    let mut accessors = Vec::new();

    let named = match fields {
        Fields::Named(named) => named,
        _ => return Ok(accessors),
    };

    for field in &named.named {
        let field_name = match &field.ident {
            Some(name) => name.clone(),
            None => continue,
        };

        let mut get = get_all;
        let mut set = false;

        for attr in &field.attrs {
            if !attr.path().is_ident("pyo3") {
                continue;
            }
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("get") {
                    get = true;
                } else if meta.path.is_ident("set") {
                    set = true;
                } else if meta.path.is_ident("name") {
                    let _ = meta.value()?.parse::<syn::LitStr>()?;
                }
                Ok(())
            })?;
        }

        if get || set {
            accessors.push(FieldAccessor {
                name: field_name,
                ty: field.ty.clone(),
                get,
                set,
            });
        }
    }

    Ok(accessors)
}

fn strip_pyo3_attrs(fields: &mut Fields) {
    let named = match fields {
        Fields::Named(named) => named,
        _ => return,
    };

    for field in &mut named.named {
        field.attrs.retain(|attr| !attr.path().is_ident("pyo3"));
    }
}

/// Generate the `Pyo3Accessors` trait implementation.
fn generate_accessor_impl(struct_name: &syn::Ident, accessors: &[FieldAccessor]) -> TokenStream {
    if accessors.is_empty() {
        return quote! {
            impl ::pyo3::Pyo3Accessors for #struct_name {
                fn __pyo3_register_accessors(
                    _ctx: &::rustpython_vm::Context,
                    _class: &'static ::rustpython_vm::Py<::rustpython_vm::builtins::PyType>,
                ) {}
            }
        };
    }

    let registrations = accessors.iter().map(|acc| {
        let field_name = &acc.name;
        let field_name_str = field_name.to_string();
        let field_ty = &acc.ty;

        let getter = if acc.get {
            quote! {
                .with_get(|obj: &#struct_name, vm: &::rustpython_vm::VirtualMachine| -> ::rustpython_vm::PyResult {
                    let py = ::pyo3::Python::from_vm(vm);
                    Ok(::pyo3::ToPyObject::to_object(&obj.#field_name, py).unbind().into_object())
                })
            }
        } else {
            quote! {}
        };

        let setter = if acc.set {
            quote! {
                .with_set(|obj: &#struct_name, value: #field_ty| {
                    // FIXME: technically UB under Rust's aliasing model.
                    //
                    // RustPython's PyGetSet::with_set provides `&Self`
                    // (immutable), but we need to mutate the field.
                    // In practice this is sound because:
                    //   1. RustPython is single-threaded (no concurrent access)
                    //   2. The reference comes from the heap-allocated payload
                    //      and no other code holds a reference during the setter
                    //
                    // The proper fix is to use interior mutability (Cell/RefCell)
                    // or a RustPython-native mutable access path. Tracked for
                    // Phase 2 when we implement PyCell-equivalent functionality.
                    unsafe {
                        let obj_mut = &mut *(obj as *const #struct_name as *mut #struct_name);
                        obj_mut.#field_name = value;
                    }
                })
            }
        } else {
            quote! {}
        };

        quote! {
            {
                let getset = ::rustpython_vm::builtins::PyGetSet::new(
                    #field_name_str.to_string(),
                    class,
                )
                #getter
                #setter;
                let getset_ref: ::rustpython_vm::PyRef<::rustpython_vm::builtins::PyGetSet> =
                    ctx.new_pyref(getset);
                class.set_str_attr(
                    #field_name_str,
                    getset_ref,
                    ctx,
                );
            }
        }
    });

    quote! {
        impl ::pyo3::Pyo3Accessors for #struct_name {
            fn __pyo3_register_accessors(
                ctx: &::rustpython_vm::Context,
                class: &'static ::rustpython_vm::Py<::rustpython_vm::builtins::PyType>,
            ) {
                use ::rustpython_vm::AsObject;
                #(#registrations)*
            }
        }
    }
}

pub fn expand_enum(attr: TokenStream, mut input: ItemEnum) -> Result<TokenStream> {
    let options = parse_pyclass_options(&attr);

    let enum_name_str = options
        .name
        .clone()
        .unwrap_or_else(|| input.ident.to_string());

    strip_pyo3_attrs_enum(&mut input);

    let enum_name = &input.ident;
    let inventory_class_name = inventory_class_name(enum_name);
    let inventory_def = generate_inventory_class(&inventory_class_name);
    let pyclass_flags = pyclass_impl_flags(&options);

    let has_debug = input.attrs.iter().any(|attr| {
        if attr.path().is_ident("derive") {
            attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            )
            .map(|paths| {
                paths.iter().any(|p| {
                    p.is_ident("Debug") || p.segments.last().map_or(false, |s| s.ident == "Debug")
                })
            })
            .unwrap_or(false)
        } else {
            false
        }
    });

    let debug_impl = if has_debug {
        quote! {}
    } else {
        quote! {
            impl ::core::fmt::Debug for #enum_name {
                fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    f.debug_struct(#enum_name_str).finish_non_exhaustive()
                }
            }
        }
    };

    Ok(quote! {
        #[::rustpython_vm::pyclass(module = false, name = #enum_name_str)]
        #input

        #debug_impl

        impl ::rustpython_vm::PyPayload for #enum_name {
            #[inline]
            fn class(ctx: &::rustpython_vm::Context) -> &'static ::rustpython_vm::Py<::rustpython_vm::builtins::PyType> {
                let already_init = <Self as ::rustpython_vm::class::StaticType>::static_cell().get().is_some();
                let _ = ctx;
                <Self as ::rustpython_vm::class::PyClassImpl>::make_static_type();
                let typ = <Self as ::rustpython_vm::class::StaticType>::static_type();
                if !already_init {
                    for __pyo3_items in ::pyo3::inventory::iter::<#inventory_class_name> {
                        let __pyo3_items = ::pyo3::Pyo3ClassInventory::items(__pyo3_items);
                        (__pyo3_items.extend_class)(ctx, typ);
                        typ.extend_methods(__pyo3_items.methods, ctx);
                        ::pyo3::slots::apply_inventory_slots(typ, __pyo3_items.extend_slots);
                    }
                    ::pyo3::slots::fixup_dunder_slots(typ, ctx);
                }
                typ
            }
        }

        impl<'py> ::pyo3::IntoPyObject<'py> for #enum_name {
            type Target = ::pyo3::types::PyAny;
            type Error = ::pyo3::PyErr;

            fn into_pyobject(self, py: ::pyo3::Python<'py>) -> Result<::pyo3::Bound<'py, Self::Target>, Self::Error> {
                let obj = ::rustpython_vm::convert::ToPyObject::to_pyobject(self, py.vm());
                Ok(::pyo3::Bound::from_object(py, obj))
            }
        }

        impl ::pyo3::PyTypeInfo for #enum_name {
            const NAME: &'static str = #enum_name_str;
            const MODULE: Option<&'static str> = None;
        }

        impl ::pyo3::Pyo3Accessors for #enum_name {
            fn __pyo3_register_accessors(
                _ctx: &::rustpython_vm::Context,
                _class: &'static ::rustpython_vm::Py<::rustpython_vm::builtins::PyType>,
            ) {}
        }

        impl ::pyo3::Pyo3BasePayload for #enum_name {
            type BasePayload = ::rustpython_vm::builtins::PyBaseObject;
        }

        impl ::rustpython_vm::class::PyClassImpl for #enum_name {
            const TP_FLAGS: ::rustpython_vm::types::PyTypeFlags = #pyclass_flags;

            fn impl_extend_class(
                _ctx: &'static ::rustpython_vm::Context,
                _class: &'static ::rustpython_vm::Py<::rustpython_vm::builtins::PyType>,
            ) {}

            const METHOD_DEFS: &'static [::rustpython_vm::function::PyMethodDef] = &[];

            fn extend_slots(_slots: &mut ::rustpython_vm::types::PyTypeSlots) {}
        }

        #inventory_def
    })
}

fn inventory_class_name(ident: &syn::Ident) -> syn::Ident {
    format_ident!("__Pyo3InventoryFor{}", ident)
}

fn generate_inventory_class(inventory_class_name: &syn::Ident) -> TokenStream {
    quote! {
        #[doc(hidden)]
        pub struct #inventory_class_name {
            items: ::pyo3::Pyo3ClassItems,
        }

        impl #inventory_class_name {
            pub const fn new(items: ::pyo3::Pyo3ClassItems) -> Self {
                Self { items }
            }
        }

        impl ::pyo3::Pyo3ClassInventory for #inventory_class_name {
            fn items(&'static self) -> &'static ::pyo3::Pyo3ClassItems {
                &self.items
            }
        }

        ::pyo3::inventory::collect!(#inventory_class_name);
    }
}

fn pyclass_impl_flags(options: &PyClassOptions) -> TokenStream {
    let mut flags = quote! {
        {
            #[cfg(not(debug_assertions))]
            {
                ::rustpython_vm::types::PyTypeFlags::DEFAULT
            }
            #[cfg(debug_assertions)]
            {
                ::rustpython_vm::types::PyTypeFlags::DEFAULT
                    .union(::rustpython_vm::types::PyTypeFlags::_CREATED_WITH_FLAGS)
            }
        }
    };
    if options.subclass {
        flags = quote! { #flags.union(::rustpython_vm::types::PyTypeFlags::BASETYPE) };
    }
    if options.sequence {
        flags = quote! { #flags.union(::rustpython_vm::types::PyTypeFlags::SEQUENCE) };
    }
    flags
}

fn strip_pyo3_attrs_enum(input: &mut ItemEnum) {
    for variant in &mut input.variants {
        variant.attrs.retain(|attr| !attr.path().is_ident("pyo3"));
        match &mut variant.fields {
            Fields::Named(named) => {
                for field in &mut named.named {
                    field.attrs.retain(|attr| !attr.path().is_ident("pyo3"));
                }
            }
            Fields::Unnamed(unnamed) => {
                for field in &mut unnamed.unnamed {
                    field.attrs.retain(|attr| !attr.path().is_ident("pyo3"));
                }
            }
            Fields::Unit => {}
        }
    }
}
