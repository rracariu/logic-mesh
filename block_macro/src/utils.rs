// Copyright (c) 2022-2023, Radu Racariu.

use std::collections::BTreeMap;
use syn::{Attribute, Expr, ExprLit, Lit, Meta, Type, TypePath};

///
/// Extract the block field names and their types.
///
pub(super) fn get_block_fields(ast: &syn::DeriveInput) -> BTreeMap<String, Type> {
    let mut members = BTreeMap::<String, Type>::new();

    if let syn::Data::Struct(struct_data) = &ast.data {
        if let syn::Fields::Named(fields) = &struct_data.fields {
            for field in &fields.named {
                if let Some(id) = &field.ident {
                    if let syn::Type::Path(_) = &field.ty {
                        members.insert(id.to_string(), field.ty.clone());
                    }
                }
            }
        }
    };

    members
}

///
/// Get all the inputs fields and their attributes
///
pub(super) fn get_block_inputs_props(
    ast: &syn::DeriveInput,
) -> Vec<(String, BTreeMap<String, String>)> {
    get_block_field_props(ast, "input")
}

///
/// Get all the output fields and their attributes
///
pub(super) fn get_block_outputs_props(
    ast: &syn::DeriveInput,
) -> Vec<(String, BTreeMap<String, String>)> {
    get_block_field_props(ast, "output")
}

///
/// Extract block props helper attributes
/// as a map of strings
///
pub(super) fn get_block_attributes(ast: &syn::DeriveInput) -> BTreeMap<String, String> {
    let mut attrs: BTreeMap<String, String> = BTreeMap::new();
    ast.attrs
        .iter()
        .filter_map(|attr| {
            let Meta::NameValue(nv) = &attr.meta else {
                return None;
            };
            let id = nv.path.get_ident()?;
            if let Expr::Lit(ExprLit {
                lit: Lit::Str(val), ..
            }) = &nv.value
            {
                Some((id.to_string(), val.value()))
            } else {
                None
            }
        })
        .for_each(|(id, val)| {
            let val = val.trim().to_string();

            if let Some(prev_val) = attrs.get(&id) {
                attrs.insert(id, (*prev_val).clone() + &val);
            } else {
                attrs.insert(id, val);
            }
        });
    attrs
}

///
/// Resolve the crate path used in the generated code.
///
/// Defaults to `::logic_mesh`; overridden with the provided escape hatch
/// `#[logic_mesh(crate = "path")]` when the dependency is renamed.
///
pub(super) fn get_crate_path(ast: &syn::DeriveInput) -> syn::Path {
    for attr in &ast.attrs {
        if !attr.path().is_ident("logic_mesh") {
            continue;
        }

        let mut path: Option<syn::Path> = None;
        let result = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("crate") {
                let lit: syn::LitStr = meta.value()?.parse()?;
                path = Some(lit.parse()?);
                Ok(())
            } else {
                Err(meta.error("unsupported attribute; expected `crate = \"path\"`"))
            }
        });

        if let Err(err) = result {
            panic!("invalid #[logic_mesh(...)] attribute: {err}");
        }

        if let Some(path) = path {
            return path;
        }
    }

    syn::parse_quote!(::logic_mesh)
}

///
/// Extract the attribute props for an input attribute
///
/// These would be:
///
/// - kind: A String property for the Haystack Kind for the input
/// - count The number of inputs to be created.
///
pub(super) fn get_block_input_attribute(ast: &syn::DeriveInput) -> BTreeMap<String, String> {
    let mut attrs: BTreeMap<String, String> = BTreeMap::new();

    ast.attrs
        .iter()
        .filter(|attr| matches!(attr.path().get_ident(), Some(id) if id == "input"))
        .for_each(|attr| get_attribute_props(attr, &mut attrs));

    attrs
}

///
/// Extract the attribute props for an helper attribute
///
fn get_attribute_props(input_attr: &syn::Attribute, attrs: &mut BTreeMap<String, String>) {
    if !matches!(&input_attr.meta, Meta::List(_)) {
        return;
    }
    let _ = input_attr.parse_nested_meta(|meta| {
        let Some(id) = meta.path.get_ident() else {
            return Ok(());
        };
        let Ok(lit) = meta.value().and_then(|v| v.parse::<Lit>()) else {
            return Ok(());
        };
        match lit {
            Lit::Str(lit) => {
                attrs.insert(id.to_string(), lit.value());
            }
            Lit::Int(lit) => {
                attrs.insert(id.to_string(), lit.to_string());
            }
            _ => {}
        }
        Ok(())
    });
}

// Input/output fields are detected by the type name, matched exactly on
// the last path segment so both bare `InputImpl` and qualified paths like
// `logic_mesh::blocks::InputImpl` are recognized, while unrelated types
// that merely share the prefix (e.g. `InputImplConfig`) are not.
fn field_type_is(ty: &TypePath, field_type: &str) -> bool {
    let it = match field_type {
        "input" => "InputImpl",
        "output" => "OutputImpl",
        _ => panic!("Invalid field type."),
    };

    ty.path.segments.last().is_some_and(|seg| seg.ident == it)
}

fn filed_attribute_is(attr: &Attribute, field_type: &str) -> bool {
    attr.path()
        .get_ident()
        .filter(|id| id.to_string().as_str() == field_type)
        .is_some()
}

fn get_block_field_props(
    ast: &syn::DeriveInput,
    field_type: &str,
) -> Vec<(String, BTreeMap<String, String>)> {
    let mut props: Vec<(String, BTreeMap<String, String>)> = Vec::new();

    if let syn::Data::Struct(struct_data) = &ast.data {
        if let syn::Fields::Named(fields) = &struct_data.fields {
            for field in &fields.named {
                if let Some(field_name) = field.ident.as_ref() {
                    if let syn::Type::Path(ty) = &field.ty {
                        if field_type_is(ty, field_type) {
                            let mut attr_props: BTreeMap<String, String> = BTreeMap::new();
                            for attr in field
                                .attrs
                                .iter()
                                .filter(|attr| filed_attribute_is(attr, field_type))
                            {
                                get_attribute_props(attr, &mut attr_props);
                            }

                            let field_name = field_name.to_string();
                            if !attr_props.is_empty()
                                && !props.iter().any(|(name, _)| name == &field_name)
                            {
                                props.push((field_name, attr_props));
                            }
                        }
                    }
                }
            }
        }
    }

    props
}
