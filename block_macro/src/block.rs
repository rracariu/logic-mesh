// Copyright (c) 2022-2023, Radu Racariu.

use proc_macro::TokenStream;
use syn::{parse::Parser, parse_macro_input, DeriveInput};

use crate::utils::{get_block_input_attribute, get_crate_path};

/// Implements the `block` attribute macro.
///
/// Adds the `id`, `name`, and `state` members to the struct.
/// Also adds the `inputs` member if the struct has inputs.
pub(super) fn block_impl(input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);

    let props = get_block_input_attribute(&ast);
    let krate = get_crate_path(&ast);

    match &mut ast.data {
        syn::Data::Struct(ref mut struct_data) => {
            if let syn::Fields::Named(fields) = &mut struct_data.fields {
                // Add the `id` member
                fields.named.push(
                    syn::Field::parse_named
                        .parse2(quote! { #[allow(missing_docs)] id: #krate::Uuid })
                        .unwrap(),
                );

                // Add the `state` member
                fields.named.push(
                    syn::Field::parse_named
                        .parse2(quote! { #[allow(missing_docs)] state: #krate::base::block::BlockState })
                        .unwrap(),
                );

                // Add the inputs fields for block declared inputs
                if !props.is_empty() {
                    fields.named.push(
                        syn::Field::parse_named
                            .parse2(quote! { #[allow(missing_docs)] _inputs: Vec::<#krate::blocks::InputImpl> })
                            .expect("input props"),
                    )
                }
            }

            quote! {
                #[allow(missing_docs)]
                #ast
            }
            .into()
        }
        _ => panic!("`block` attribute has to be used with structs "),
    }
}
