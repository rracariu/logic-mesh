// Copyright (c) 2022-2023, IntriSemantics Corp.

use proc_macro::TokenStream;
use syn::{parse::Parser, parse_macro_input, DeriveInput};

use crate::utils::get_block_input_attribute;

pub(super) fn block_impl(input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);

    let props = get_block_input_attribute(&ast);

    match &mut ast.data {
        syn::Data::Struct(ref mut struct_data) => {
            if let syn::Fields::Named(fields) = &mut struct_data.fields {
                // Add the `id` member
                fields
                    .named
                    .push(syn::Field::parse_named.parse2(quote! { id: Uuid }).unwrap());

                // Add the `state` member
                fields.named.push(
                    syn::Field::parse_named
                        .parse2(quote! { state: BlockState })
                        .unwrap(),
                );

                // Add the inputs fields for block declared inputs
                if !props.is_empty() {
                    fields.named.push(
                        syn::Field::parse_named
                            .parse2(quote! { _inputs: Vec::<InputImpl> })
                            .expect("input props"),
                    )
                }
            }

            quote! {
                #ast
            }
            .into()
        }
        _ => panic!("`block` attribute has to be used with structs "),
    }
}
