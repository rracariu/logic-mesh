// Copyright (c) 2022-2023, IntriSemantics Corp.

use proc_macro::TokenStream;

use crate::utils::{get_attributes_map, get_block_input_attribute};

pub(super) fn block_props_impl(ast: &syn::DeriveInput) -> TokenStream {
    let name = &ast.ident;

    let block_props_attrs = get_attributes_map(ast);
    let prop_names = block_props_attrs.keys().map(|k| format_ident!("{}", k));
    let prop_values = block_props_attrs.values();

    let block_desc = format_ident!("_{}_DESC", name.to_string().to_uppercase());
    let input_init = create_input_init(ast);

    let tokens = quote! {

        use lazy_static::lazy_static;

        lazy_static! {
            static ref #block_desc: BlockDesc = {
                let desc = BlockDesc {
                    #(#prop_names : #prop_values.to_string(),)*
                };

                desc
            };
        }

        impl #name {
            pub fn new() -> Self {
                Self {
                    id:  Uuid::new_v4(),
                    state: BlockState::Stopped,
                    period: InputImpl::new("period", HaystackKind::Number),
                    out:  OutputImpl::new(HaystackKind::Number),
                    #input_init
                }
            }
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

fn create_input_init(ast: &syn::DeriveInput) -> proc_macro2::TokenStream {
    let block_input_props = get_block_input_attribute(ast);

    if block_input_props.is_empty() {
        proc_macro2::TokenStream::default()
    } else {
        let kind = format_ident!(
            "{}",
            block_input_props
                .get("kind")
                .cloned()
                .unwrap_or("Null".into())
        );

        let name = block_input_props
            .get("name")
            .cloned()
            .unwrap_or("in".into());

        let count = block_input_props
            .get("count")
            .and_then(|e| e.parse::<usize>().ok())
            .unwrap_or(1);

        let names = (0..count).map(|i| format!("{name}{i}"));

        quote! {
            _inputs: vec![ #(InputImpl::new(#names, HaystackKind::#kind)),* ]
        }
    }
}
