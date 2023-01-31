// Copyright (c) 2022-2023, IntriSemantics Corp.

use proc_macro::TokenStream;
use syn::{parse::Parser, parse_macro_input, DeriveInput};

pub(super) fn block_impl(input: TokenStream) -> TokenStream {
    let mut ast = parse_macro_input!(input as DeriveInput);
    match &mut ast.data {
        syn::Data::Struct(ref mut struct_data) => {
            match &mut struct_data.fields {
                syn::Fields::Named(fields) => {
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
                }
                _ => (),
            }

            return quote! {
                #ast
            }
            .into();
        }
        _ => panic!("`block` has to be used with structs "),
    }
}
