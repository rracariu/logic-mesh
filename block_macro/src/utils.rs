// Copyright (c) 2022-2023, Radu Racariu.

use std::collections::BTreeMap;
use syn::{Attribute, Lit, Meta, NestedMeta, TypePath};

///
/// Extract the block field names and their types.
///
pub(super) fn get_block_fields(ast: &syn::DeriveInput) -> BTreeMap<String, String> {
    let mut members = BTreeMap::<String, String>::new();

    if let syn::Data::Struct(struct_data) = &ast.data {
        if let syn::Fields::Named(fields) = &struct_data.fields {
            for field in &fields.named {
                if let Some(id) = &field.ident {
                    if let syn::Type::Path(ty) = &field.ty {
                        if let Some(ty) = ty.path.get_ident() {
                            members.insert(id.to_string(), ty.to_string());
                        }
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

fn field_type_is(ty: &TypePath, field_type: &str) -> bool {
    let it: String = match field_type {
        "input" => "InputImpl",
        "output" => "OutputImpl",
        _ => panic!("Invalid field type."),
    }
    .into();

    ty.path
        .get_ident()
        .filter(|id| id.to_string().starts_with::<&String>(&it))
        .is_some()
}

fn filed_attribute_is(attr: &Attribute, field_type: &str) -> bool {
    attr.path
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
