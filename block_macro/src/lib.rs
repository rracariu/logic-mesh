// Copyright (c) 2022-2023, IntriSemantics Corp.

extern crate proc_macro;
extern crate syn;
#[macro_use]
extern crate quote;

use std::collections::BTreeMap;

use litrs::Literal;
use proc_macro::TokenStream;

#[proc_macro_derive(BlockProps, attributes(name, lib, version))]
pub fn block_props(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();

    let gen = block_props_impl(&ast);

    //eprintln!("block_props: {gen}");

    gen
}

fn block_props_impl(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;

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

    let k = attrs.keys().map(|k| format_ident!("_{}", k.to_uppercase()));
    let v = attrs.values();

    let tokens = quote! {

        impl #name {
            #(const #k:&str = #v;)*
        }

        impl BlockProps for #name {
            type Rx = <InputImpl as InputProps>::Rx;
            type Tx = <InputImpl as InputProps>::Tx;

            fn id(&self) -> &Uuid {
                &self.id
            }

            fn desc(&self) -> &BlockDesc {
                &self.desc
            }

            fn state(&self) -> BlockState {
                BlockState::Fault
            }

            fn inputs(&mut self) -> Vec<&mut dyn Input<Rx = Self::Rx, Tx = Self::Tx>> {
                vec![&mut self.period]
            }

            fn output(&mut self) -> &mut dyn Output<Tx = Self::Tx> {
                &mut self.out
            }
        }
    };

    tokens.into()
}
