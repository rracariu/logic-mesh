// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::collections::BTreeMap;

use litrs::Literal;
use proc_macro::TokenStream;
use syn::{Lit, Meta, NestedMeta};

pub(super) fn get_attributes_map(ast: &syn::DeriveInput) -> BTreeMap<String, String> {
    let attrs: BTreeMap<String, String> = ast
        .attrs
        .iter()
        .filter_map(|attr| {
            if let Some(id) = attr.path.get_ident().map(|id| id.to_string()) {
                for tok in TokenStream::from(attr.tokens.clone()) {
                    match Literal::try_from(tok) {
                        Ok(Literal::String(lit)) => {
                            return Some((id, lit.into_value().to_string()))
                        }
                        _ => continue,
                    }
                }

                None
            } else {
                None
            }
        })
        .collect();
    attrs
}

pub(super) fn get_block_input_attribute(ast: &syn::DeriveInput) -> BTreeMap<String, String> {
    let mut attrs: BTreeMap<String, String> = BTreeMap::new();

    ast.attrs
        .iter()
        .filter(|attr| matches!(attr.path.get_ident(), Some(id) if id == "input"))
        .for_each(|attr| get_input_attribute_props(attr, &mut attrs));

    attrs
}

///
/// Extract the common input attribute props
///
/// These would be:
///
/// - Kind: A String property for the Haystack Kind for the input
/// - count The number of inputs to be created.
///
fn get_input_attribute_props(input_attr: &syn::Attribute, attrs: &mut BTreeMap<String, String>) {
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
