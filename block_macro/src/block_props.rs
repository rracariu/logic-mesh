// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::collections::BTreeMap;

use litrs::Literal;
use proc_macro::TokenStream;

pub(super) fn block_props_impl(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;

    let attrs = get_attributes_map(ast);

    let k = attrs.keys().map(|k| format_ident!("{}", k));
    let v = attrs.values();

    let block_desc = format_ident!("_{}_DESC", name.to_string().to_uppercase());

    let tokens = quote! {

        use lazy_static::lazy_static;

        lazy_static! {
            static ref #block_desc: BlockDesc = {
                let desc = BlockDesc {
                    #(#k : #v.to_string(),)*
                };

                desc
            };
        }

        impl BlockProps for #name {
            type Rx = <InputImpl as InputProps>::Rx;
            type Tx = <InputImpl as InputProps>::Tx;

            fn id(&self) -> &Uuid {
                &self.id
            }

            fn desc(&self) -> &BlockDesc {
                &#block_desc
            }

            fn state(&self) -> BlockState {
                self.state
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

fn get_attributes_map(ast: &syn::DeriveInput) -> BTreeMap<String, String> {
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
