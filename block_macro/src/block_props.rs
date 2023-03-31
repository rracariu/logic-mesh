// Copyright (c) 2022-2023, IntriSemantics Corp.

use std::collections::BTreeMap;

use proc_macro::TokenStream;
use proc_macro2::{Group, TokenTree};

use crate::utils::{
    get_block_attributes, get_block_fields, get_block_input_attribute, get_block_inputs_props,
    get_block_outputs_props,
};

///
/// Generates the implementation for the BlockProps trait
/// and creates the constructor function.
///
/// It also initializes any block fields that are not inputs or output
/// to their default type value.
///
pub(super) fn block_props_impl(ast: &syn::DeriveInput) -> TokenStream {
    let block_ident = &ast.ident;

    // Block input attributes (defined by the inputs attribute of the block)
    let block_defined_inputs = get_block_input_attribute(ast);
    let block_defined_init = create_block_defined_input_init(&block_defined_inputs);

    // Block inputs
    let block_input_props = get_block_inputs_props(ast);
    let input_fields_init = create_block_input_fields_init(&block_input_props);
    let inputs_mut_refs =
        create_input_members_ref(!block_defined_inputs.is_empty(), &block_input_props, true);

    let inputs_refs =
        create_input_members_ref(!block_defined_inputs.is_empty(), &block_input_props, false);

    // Block outputs
    let block_outputs_props = get_block_outputs_props(ast);
    let outputs_field_init = create_block_outputs_field_init(&block_outputs_props);
    let outputs_mut_ref = create_outputs_member_ref(&block_outputs_props, true);
    let outputs_ref = create_outputs_member_ref(&block_outputs_props, false);

    // Block description attributes
    let mut block_props_attrs = get_block_attributes(ast);
    if !block_props_attrs.contains_key("doc") {
        block_props_attrs.insert("doc".to_string(), "".to_string());
    }

    let block_prop_names = block_props_attrs.keys().map(|name| format_ident!("{name}"));
    let block_prop_values = block_props_attrs.values();

    // Init other block fields that are not the reserved fields (id, name, state) or inputs/output to their default value
    let block_fields = get_block_fields(ast);
    let block_field_init =
        create_block_fields_init(&block_fields, &block_input_props, &block_outputs_props);

    // Create the code for getting input and output description
    let input_desc = create_input_desc(&block_defined_inputs, &block_input_props);
    let out_desc = create_output_desc(&block_outputs_props);

    // The code that gets generated for the blocks
    let tokens = quote! {

        // Generated constructors
        impl #block_ident {
            pub fn new(name: &str) -> Self {
                let uuid = Uuid::new_v4();
                Self::new_uuid(name, uuid)
            }

            pub fn new_uuid(name: &str, uuid: Uuid) -> Self {
                Self {
                    id: uuid,
                    name: name.to_string(),
                    state: BlockState::Stopped,
                    #block_field_init
                    #outputs_field_init,
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

            fn desc(&self) -> &'static BlockDesc {
                <Self as crate::base::block::BlockDescAccess>::desc()
            }

            fn state(&self) -> BlockState {
                self.state
            }

            fn set_state(&mut self, state: BlockState) -> BlockState {
                self.state = state;
                self.state
            }

            fn inputs(&self) -> Vec<&dyn Input<Rx = Self::Rx, Tx = Self::Tx>> {
                #inputs_refs
            }

            fn inputs_mut(&mut self) -> Vec<&mut dyn Input<Rx = Self::Rx, Tx = Self::Tx>> {
                #inputs_mut_refs
            }

            fn outputs_mut(&mut self) -> Vec<&mut dyn Output<Tx = Self::Tx>> {
                #outputs_mut_ref
            }

            fn outputs(&self) -> Vec<&dyn Output<Tx = Self::Tx>> {
                #outputs_ref
            }

            fn links(&self) -> Vec<&dyn crate::base::link::Link> {
                let mut res = Vec::new();
                self.outputs().iter().for_each(|out| res.append(&mut out.links()));
                res
            }

            fn remove_link(&mut self, link: &dyn crate::base::link::Link) {
                self.outputs_mut().iter_mut().for_each(|out| out.remove_link(link))
            }
        }

        // Implementation of the BlockDesc trait
        // using the attributes
        impl crate::base::block::BlockDescAccess for #block_ident {
            fn desc() -> &'static BlockDesc {
                lazy_static::lazy_static! {
                    static ref DESC: BlockDesc = {
                        use crate::base::block::BlockDesc;
                        use crate::base::block::props::BlockPin;

                        let desc = BlockDesc {
                            #(#block_prop_names : #block_prop_values.to_string(),)*
                            #out_desc,
                            #input_desc
                        };

                        desc
                    };
                }
                &*DESC
            }
        }
    };

    tokens.into()
}

// Init the input fields of a block
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

// Init custom fields that a user my have on a block
fn create_block_fields_init(
    block_fields: &BTreeMap<String, String>,
    block_input_props: &BTreeMap<String, BTreeMap<String, String>>,
    block_output_props: &BTreeMap<String, BTreeMap<String, String>>,
) -> proc_macro2::TokenStream {
    if block_fields.is_empty() {
        proc_macro2::TokenStream::default()
    } else {
        let filter = |field_name: &&String| {
            !block_input_props.contains_key(*field_name)
                && !block_output_props.contains_key(*field_name)
                && field_name.as_str() != "id"
                && field_name.as_str() != "name"
                && field_name.as_str() != "state"
        };

        let field = block_fields
            .keys()
            .filter(filter)
            .map(|field_name| format_ident!("{field_name}"));

        let ty = block_fields
            .iter()
            .filter(|(k, _)| filter(k))
            .map(|(_, ty)| format_ident!("{ty}"));

        quote! {
            #(#field: #ty::default(),)*
        }
    }
}

// Init automatic inputs defined on the block attribure
fn create_block_defined_input_init(
    block_defined_input_props: &BTreeMap<String, String>,
) -> proc_macro2::TokenStream {
    if block_defined_input_props.is_empty() {
        proc_macro2::TokenStream::default()
    } else {
        let kind = format_ident!(
            "{}",
            block_defined_input_props
                .get("kind")
                .cloned()
                .unwrap_or("Null".into())
        );

        let name = block_defined_input_props
            .get("name")
            .cloned()
            .unwrap_or("in".into());

        let count = block_defined_input_props
            .get("count")
            .and_then(|e| e.parse::<usize>().ok())
            .unwrap_or(0);

        let names = (0..count).map(|i| format!("{name}{i}"));

        quote! {
            _inputs: vec![ #(InputImpl::new(#names, HaystackKind::#kind, uuid.clone())),* ],
        }
    }
}

// Create the outputS fieLd init
fn create_block_outputs_field_init(
    block_output_props: &BTreeMap<String, BTreeMap<String, String>>,
) -> proc_macro2::TokenStream {
    if block_output_props.is_empty() || block_output_props.len() > 1 {
        panic!("Block must have exactly one output field.")
    } else {
        let out_field = block_output_props.keys().map(|id| format_ident!("{id}"));

        let output_name = block_output_props
            .iter()
            .map(|(name, props)| props.get("name").cloned().unwrap_or(name.clone()));

        let kind = block_output_props.iter().map(|(_, props)| {
            format_ident!("{}", props.get("kind").cloned().unwrap_or("Null".into()))
        });

        quote! {
            #(#out_field: OutputImpl::new_named(#output_name, HaystackKind::#kind, uuid)),*
        }
    }
}

// Create the reference for input fields for all the types of inputs:
// block defined automatic inputs or user defined block input fields
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

// Create the reference for the output field of the block
fn create_outputs_member_ref(
    block_output_props: &BTreeMap<String, BTreeMap<String, String>>,
    mutable: bool,
) -> proc_macro2::TokenStream {
    if block_output_props.is_empty() {
        quote! {
            Vec::default()
        }
    } else {
        let output_fields = block_output_props.keys().map(|id| format_ident!("{id}"));

        let borrow = if mutable {
            TokenTree::from(format_ident!("mut"))
        } else {
            TokenTree::Group(Group::new(
                proc_macro2::Delimiter::None,
                TokenStream::default().into(),
            ))
        };

        quote! {
            vec![ #(&#borrow self.#output_fields),* ]
        }
    }
}

// Create the description of the input fields
fn create_input_desc(
    block_defined_input_props: &BTreeMap<String, String>,
    block_input_props: &BTreeMap<String, BTreeMap<String, String>>,
) -> proc_macro2::TokenStream {
    let input_field_names = block_input_props.keys();

    let input_field_kinds = block_input_props
        .iter()
        .map(|(_, props)| format_ident!("{}", props.get("kind").cloned().unwrap_or("Null".into())));

    let kind = format_ident!(
        "{}",
        block_defined_input_props
            .get("kind")
            .cloned()
            .unwrap_or("Null".into())
    );

    let name = block_defined_input_props
        .get("name")
        .cloned()
        .unwrap_or("in".into());

    let count = block_defined_input_props
        .get("count")
        .and_then(|e| e.parse::<usize>().ok())
        .unwrap_or(0);

    let block_defined_inputs = (0..count).map(|i| format!("{name}{i}"));

    quote! {
        inputs: vec![#(BlockPin { name: #input_field_names.to_string(), kind: HaystackKind::#input_field_kinds },)*
        #(BlockPin { name: #block_defined_inputs.to_string(), kind: HaystackKind::#kind },)*],
    }
}

// Create the description of the outputs field
fn create_output_desc(
    block_output_props: &BTreeMap<String, BTreeMap<String, String>>,
) -> proc_macro2::TokenStream {
    let output_names = block_output_props.keys();

    let output_kinds = block_output_props
        .iter()
        .map(|(_, props)| format_ident!("{}", props.get("kind").cloned().unwrap_or("Null".into())));

    quote! {
        outputs: vec![#(BlockPin { name: #output_names.to_string(), kind: HaystackKind::#output_kinds },)*]
    }
}
