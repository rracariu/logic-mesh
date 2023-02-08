// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::collections::BTreeMap;

use syn::{Lit, Meta, NestedMeta};

pub(super) fn get_block_inputs_props(
    ast: &syn::DeriveInput,
) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut props: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();

    if let syn::Data::Struct(struct_data) = &ast.data {
        if let syn::Fields::Named(fields) = &struct_data.fields {
            for field in &fields.named {
                if let Some(field_name) = field.ident.as_ref() {
                    if let syn::Type::Path(ty) = &field.ty {
                        if ty
                            .path
                            .get_ident()
                            .filter(|id| id.to_string().starts_with::<&String>(&"InputImpl".into()))
                            .is_some()
                        {
                            for attr in field.attrs.iter().filter(|attr| {
                                attr.path
                                    .get_ident()
                                    .filter(|id| id.to_string().as_str() == "input")
                                    .is_some()
                            }) {
                                let mut attr_props: BTreeMap<String, String> = BTreeMap::new();
                                get_attribute_props(&attr, &mut attr_props);

                                if !attr_props.is_empty() {
                                    props.insert(field_name.to_string(), attr_props);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    props
}

///
/// Extract block props helper attributes
/// as a map of strings
///
pub(super) fn get_block_attributes(ast: &syn::DeriveInput) -> BTreeMap<String, String> {
    let attrs: BTreeMap<String, String> = ast
        .attrs
        .iter()
        .filter_map(|attr| {
            if let Ok(Meta::NameValue(nv)) = attr.parse_meta() {
                if let Some(id) = nv.path.get_ident() {
                    if let Lit::Str(val) = nv.lit {
                        Some((id.to_string(), val.value()))
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    attrs
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
        .filter(|attr| matches!(attr.path.get_ident(), Some(id) if id == "input"))
        .for_each(|attr| get_attribute_props(attr, &mut attrs));

    attrs
}

///
/// Extract the attribute props for an helper attribute
///
fn get_attribute_props(input_attr: &syn::Attribute, attrs: &mut BTreeMap<String, String>) {
    if let Ok(Meta::List(list)) = input_attr.parse_meta() {
        list.nested.iter().for_each(|e| {
            if let NestedMeta::Meta(Meta::NameValue(name_value)) = e {
                if let Some(id) = name_value.path.get_ident() {
                    match &name_value.lit {
                        Lit::Str(lit) => {
                            attrs.insert(id.to_string(), lit.value());
                        }

                        Lit::Int(lit) => {
                            attrs.insert(id.to_string(), lit.to_string());
                        }

                        _ => (),
                    }
                }
            }
        })
    }
}
