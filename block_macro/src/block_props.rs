// Copyright (c) 2022-2023, IntriSemantics Corp.

use proc_macro::TokenStream;

use crate::utils::{get_block_attributes, get_block_input_attribute, get_block_inputs_props};

pub(super) fn block_props_impl(ast: &syn::DeriveInput) -> TokenStream {
    let block_ident = &ast.ident;

    let input_init = create_block_anonymous_input_init(ast);
    let input_fields_init = create_block_input_fields_init(ast);

    let block_props_attrs = get_block_attributes(ast);
    let prop_names = block_props_attrs.keys().map(|name| format_ident!("{name}"));
    let prop_values = block_props_attrs.values();
    let block_desc = format_ident!("_{}_DESC", block_ident.to_string().to_uppercase());

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

        impl #block_ident {
            pub fn new() -> Self {
                Self {
                    id:  Uuid::new_v4(),
                    state: BlockState::Stopped,

                    out:  OutputImpl::new(HaystackKind::Number),
                    #input_init
                    #input_fields_init
                }
            }
        }

        impl BlockProps for #block_ident {
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

fn create_block_input_fields_init(ast: &syn::DeriveInput) -> proc_macro2::TokenStream {
    let block_input_props = get_block_inputs_props(ast);

    if block_input_props.is_empty() {
        proc_macro2::TokenStream::default()
    } else {
        let input_field = block_input_props.keys().map(|k| format_ident!("{k}"));
        let input_name = block_input_props.keys();

        let kind = block_input_props.iter().map(|(_, props)| {
            format_ident!("{}", props.get("kind").cloned().unwrap_or("Null".into()))
        });

        quote! {
            #(#input_field: InputImpl::new(#input_name, HaystackKind::#kind)),*
        }
    }
}

fn create_block_anonymous_input_init(ast: &syn::DeriveInput) -> proc_macro2::TokenStream {
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
            _inputs: vec![ #(InputImpl::new(#names, HaystackKind::#kind)),* ],
        }
    }
}
