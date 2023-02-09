// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::collections::BTreeMap;

use proc_macro::TokenStream;
use proc_macro2::{Group, TokenTree};

use crate::utils::{
    get_block_attributes, get_block_input_attribute, get_block_inputs_props, get_block_output_props,
};

pub(super) fn block_props_impl(ast: &syn::DeriveInput) -> TokenStream {
    let block_ident = &ast.ident;

    // Block input attributes
    let block_defined_inputs = get_block_input_attribute(ast);
    let block_defined_init = create_block_defined_input_init(&block_defined_inputs);

    let block_input_props = get_block_inputs_props(ast);
    let input_fields_init = create_block_input_fields_init(&block_input_props);
    let input_mut_refs =
        create_input_members_ref(!block_defined_inputs.is_empty(), &block_input_props, true);

    let input_refs =
        create_input_members_ref(!block_defined_inputs.is_empty(), &block_input_props, false);

    // Block output
    let block_output_props = get_block_output_props(ast);
    let output_field_init = create_block_output_field_init(&block_output_props);
    let out_ref = create_output_member_ref(&block_output_props);

    // Block description attributes
    let block_props_attrs = get_block_attributes(ast);
    let prop_names = block_props_attrs.keys().map(|name| format_ident!("{name}"));
    let prop_values = block_props_attrs.values();
    let block_desc = format_ident!("_{}_DESC", block_ident.to_string().to_uppercase());

    // The code that gets generated for the blocks
    let tokens = quote! {

        use lazy_static::lazy_static;

        // Accessor for block static properties
        lazy_static! {
            static ref #block_desc: BlockDesc = {
                let desc = BlockDesc {
                    #(#prop_names : #prop_values.to_string(),)*
                };

                desc
            };
        }

        // Generated constructor
        impl #block_ident {
            pub fn new(name: &str) -> Self {
                let uuid = Uuid::new_v4();
                Self {
                    id: uuid,
                    name: name.to_string(),
                    state: BlockState::Stopped,

                    #output_field_init,
                    #block_defined_init
                    #input_fields_init
                }
            }
        }

        // Implementation of the BlockProps trait
        // using the attributes
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

            fn set_state(&mut self, state: BlockState) -> BlockState {
                self.state = state;
                self.state
            }

            fn inputs(&self) -> Vec<&dyn Input<Rx = Self::Rx, Tx = Self::Tx>> {
                #input_refs
            }

            fn inputs_mut(&mut self) -> Vec<&mut dyn Input<Rx = Self::Rx, Tx = Self::Tx>> {
                #input_mut_refs
            }

            fn output_mut(&mut self) -> &mut dyn Output<Tx = Self::Tx> {
                &mut #out_ref
            }

            fn output(&self) -> &dyn Output<Tx = Self::Tx> {
                & #out_ref
            }
        }
    };

    tokens.into()
}

fn create_block_input_fields_init(
    block_input_props: &BTreeMap<String, BTreeMap<String, String>>,
) -> proc_macro2::TokenStream {
    if block_input_props.is_empty() {
        proc_macro2::TokenStream::default()
    } else {
        let input_field = block_input_props.keys().map(|k| format_ident!("{k}"));
        let input_name = block_input_props
            .iter()
            .map(|(name, props)| props.get("name").cloned().unwrap_or(name.clone()));

        let kind = block_input_props.iter().map(|(_, props)| {
            format_ident!("{}", props.get("kind").cloned().unwrap_or("Null".into()))
        });

        quote! {
            #(#input_field: InputImpl::new(#input_name, HaystackKind::#kind, uuid.clone())),*
        }
    }
}

fn create_block_defined_input_init(
    block_input_props: &BTreeMap<String, String>,
) -> proc_macro2::TokenStream {
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
            _inputs: vec![ #(InputImpl::new(#names, HaystackKind::#kind, uuid.clone())),* ],
        }
    }
}

fn create_block_output_field_init(
    block_output_props: &BTreeMap<String, BTreeMap<String, String>>,
) -> proc_macro2::TokenStream {
    if block_output_props.is_empty() || block_output_props.len() > 1 {
        panic!("Block must have exactly one output field.")
    } else {
        let out_field = block_output_props
            .keys()
            .map(|id| format_ident!("{id}"))
            .next();

        let output_name = block_output_props
            .iter()
            .map(|(name, props)| props.get("name").cloned().unwrap_or(name.clone()))
            .next();

        let kind = block_output_props
            .iter()
            .map(|(_, props)| {
                format_ident!("{}", props.get("kind").cloned().unwrap_or("Null".into()))
            })
            .next();

        quote! {
            #out_field: OutputImpl::new_named(#output_name, HaystackKind::#kind)
        }
    }
}

fn create_input_members_ref(
    has_block_defined_inputs: bool,
    block_input_props: &BTreeMap<String, BTreeMap<String, String>>,
    mutable: bool,
) -> proc_macro2::TokenStream {
    if !has_block_defined_inputs && block_input_props.is_empty() {
        quote! {
            Vec::default()
        }
    } else {
        let input_field = block_input_props.keys().map(|id| format_ident!("{id}"));

        let (borrow, iter) = if mutable {
            (
                TokenTree::from(format_ident!("mut")),
                TokenTree::from(format_ident!("iter_mut")),
            )
        } else {
            let empty = TokenTree::Group(Group::new(
                proc_macro2::Delimiter::None,
                TokenStream::default().into(),
            ));

            (empty, TokenTree::from(format_ident!("iter")))
        };

        if has_block_defined_inputs {
            quote! {
                let mut inputs = vec![ #(&#borrow self.#input_field as &#borrow dyn Input<Rx = Self::Rx, Tx = Self::Tx>),* ];
                inputs.extend(self._inputs.#iter().map(|input| input as &#borrow dyn Input<Rx = Self::Rx, Tx = Self::Tx>));

                inputs
            }
        } else {
            quote! {
                vec![ #(&#borrow self.#input_field),* ]
            }
        }
    }
}

fn create_output_member_ref(
    block_output_props: &BTreeMap<String, BTreeMap<String, String>>,
) -> proc_macro2::TokenStream {
    if block_output_props.is_empty() || block_output_props.len() > 1 {
        panic!("Block must have exactly one output field.")
    } else {
        let out_field = block_output_props
            .keys()
            .map(|id| format_ident!("{id}"))
            .next();

        quote! {
            self.#out_field
        }
    }
}
