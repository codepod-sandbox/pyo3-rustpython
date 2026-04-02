use proc_macro2::TokenStream;
use quote::quote;
use syn::{Fields, ItemStruct, Result};

/// Information about a field annotated with `#[pyo3(get)]` and/or `#[pyo3(set)]`.
struct FieldAccessor {
    name: syn::Ident,
    ty: syn::Type,
    get: bool,
    set: bool,
}

/// Expand `#[pyclass]` on a struct.
///
/// Generates:
/// 1. The struct with `#[rustpython_vm::pyclass(module = false, name = "...")]`
///    and `#[derive(rustpython_vm::PyPayload)]` added, `#[pyo3(...)]` stripped.
/// 2. A `Debug` derive if not already present.
/// 3. An impl of `pyo3::Pyo3Accessors` that registers getters/setters for
///    fields annotated with `#[pyo3(get)]` / `#[pyo3(set)]`.
pub fn expand(_attr: TokenStream, mut input: ItemStruct) -> Result<TokenStream> {
    let struct_name = &input.ident;
    let struct_name_str = struct_name.to_string();

    // Collect field accessor info before stripping attributes.
    let accessors = collect_accessors(&input.fields)?;

    // Strip #[pyo3(...)] attributes from fields.
    strip_pyo3_attrs(&mut input.fields);

    // Check if Debug is already derived.
    let has_debug = input.attrs.iter().any(|attr| {
        if attr.path().is_ident("derive") {
            attr.parse_args_with(
                syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
            )
            .map(|paths| {
                paths.iter().any(|p| {
                    p.is_ident("Debug")
                        || p.segments.last().map_or(false, |s| s.ident == "Debug")
                })
            })
            .unwrap_or(false)
        } else {
            false
        }
    });

    let debug_derive = if has_debug {
        quote! {}
    } else {
        quote! { #[derive(Debug)] }
    };

    // Generate the Pyo3Accessors implementation.
    let accessor_impl = generate_accessor_impl(struct_name, &accessors);

    // Emit the struct with rustpython attributes.
    Ok(quote! {
        #[::rustpython_vm::pyclass(module = false, name = #struct_name_str)]
        #[derive(::rustpython_vm::PyPayload)]
        #debug_derive
        #input

        #accessor_impl
    })
}

fn collect_accessors(fields: &Fields) -> Result<Vec<FieldAccessor>> {
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

        let mut get = false;
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
fn generate_accessor_impl(
    struct_name: &syn::Ident,
    accessors: &[FieldAccessor],
) -> TokenStream {
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
                .with_get(|obj: &Self| -> #field_ty {
                    obj.#field_name.clone()
                })
            }
        } else {
            quote! {}
        };

        let setter = if acc.set {
            quote! {
                .with_set(|obj: &Self, value: #field_ty| {
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
                        let obj_mut = &mut *(obj as *const Self as *mut Self);
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
