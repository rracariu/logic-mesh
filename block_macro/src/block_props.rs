// Copyright (c) 2022-2023, Radu Racariu.

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

    block_props_attrs.insert("name".to_string(), format!("{}", block_ident));

    if !block_props_attrs.contains_key("dis") {
        block_props_attrs.insert("dis".to_string(), format!("{}", block_ident));
    }

    if !block_props_attrs.contains_key("doc") {
        block_props_attrs.insert("doc".to_string(), "".to_string());
    }

    if !block_props_attrs.contains_key("ver") {
        block_props_attrs.insert("ver".to_string(), "1.0.0".to_string());
    }

    if !block_props_attrs.contains_key("library") {
        block_props_attrs.insert("library".to_string(), "core".to_string());
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
    ensure_unique_outputs(&block_defined_inputs, &block_outputs_props);

    // The code that gets generated for the blocks
    let tokens = quote! {

        // Generated constructors
        impl #block_ident {
            pub fn new() -> Self {
                let uuid = Uuid::new_v4();
                Self::new_uuid(uuid)
            }

            pub fn new_uuid(uuid: Uuid) -> Self {
                Self {
                    id: uuid,
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
            type Reader = <InputImpl as InputProps>::Reader;
            type Writer = <InputImpl as InputProps>::Writer;

            fn id(&self) -> &Uuid {
                &self.id
            }

            fn name(&self) -> &str {
                &self.desc().name
            }

            fn desc(&self) -> &'static BlockDesc {
                <Self as crate::base::block::BlockStaticDesc>::desc()
            }

            fn state(&self) -> BlockState {
                self.state
            }

            fn set_state(&mut self, state: BlockState) -> BlockState {
                self.state = state;
                self.state
            }

            fn inputs(&self) -> Vec<&dyn Input<Reader = Self::Reader, Writer = Self::Writer>> {
                #inputs_refs
            }

            fn inputs_mut(&mut self) -> Vec<&mut dyn Input<Reader = Self::Reader, Writer = Self::Writer>> {
                #inputs_mut_refs
            }

            fn outputs_mut(&mut self) -> Vec<&mut dyn Output<Writer = Self::Writer>> {
                #outputs_mut_ref
            }

            fn outputs(&self) -> Vec<&dyn Output<Writer = Self::Writer>> {
                #outputs_ref
            }

            fn links(&self) -> Vec<(&str, Vec<&dyn crate::base::link::Link>)> {
                let mut res = Vec::new();

                self.inputs().iter().for_each(|input| res.push((input.name(), input.links())));
                self.outputs().iter().for_each(|out| res.push((out.name(), out.links())));
                res
            }

            fn remove_link_by_id(&mut self, link_uuid: &Uuid) {
                self.inputs_mut().iter_mut().for_each(|input| input.remove_link_by_id(link_uuid));
                self.outputs_mut().iter_mut().for_each(|out| out.remove_link_by_id(link_uuid))
            }

            fn remove_all_links(&mut self) {
                self.inputs_mut().iter_mut().for_each(|input| input.remove_all_links());
                self.outputs_mut().iter_mut().for_each(|out| out.remove_all_links())
            }
        }

        // Implementation of the BlockDesc trait
        // using the attributes
        impl crate::base::block::BlockStaticDesc for #block_ident {
            fn desc() -> &'static BlockDesc {
                lazy_static::lazy_static! {
                    static ref DESC: BlockDesc = {
                        use crate::base::block::desc::BlockDesc;
                        use crate::base::block::desc::BlockPin;
                        use crate::base::block::desc::BlockImplementation;

                        let desc = BlockDesc {
                            implementation: BlockImplementation::Native,
                            run_condition: None,
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

        impl Default for #block_ident {
            fn default() -> Self {
                Self::new()
            }
        }
    };

    tokens.into()
}

/// Init the input fields of a block
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

/// Init custom fields that a user my have on a block
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

/// Init automatic inputs defined on the block attribute
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

        let count = get_block_defined_inputs_count(block_defined_input_props).unwrap_or(0);

        let names = (0..count).map(|i| format!("{name}{i}"));

        quote! {
            _inputs: vec![ #(InputImpl::new(#names, HaystackKind::#kind, uuid.clone())),* ],
        }
    }
}

/// Create the outputS fieLd init
fn create_block_outputs_field_init(
    block_output_props: &BTreeMap<String, BTreeMap<String, String>>,
) -> proc_macro2::TokenStream {
    if block_output_props.is_empty() {
        panic!("Block must have at least one output field.")
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

/// Create the reference for input fields for all the types of inputs:
/// block defined automatic inputs or user defined block input fields
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
                let mut inputs = vec![ #(&#borrow self.#input_field as &#borrow dyn Input<Reader = Self::Reader, Writer = Self::Writer>),* ];
                inputs.extend(self._inputs.#iter().map(|input| input as &#borrow dyn Input<Reader = Self::Reader, Writer = Self::Writer>));

                inputs
            }
        } else {
            quote! {
                vec![ #(&#borrow self.#input_field),* ]
            }
        }
    }
}

/// Create the reference for the output field of the block
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

/// Create the description of the input fields
fn create_input_desc(
    block_defined_input_props: &BTreeMap<String, String>,
    block_input_props: &BTreeMap<String, BTreeMap<String, String>>,
) -> proc_macro2::TokenStream {
    ensure_unique_inputs(block_defined_input_props, block_input_props);

    let input_field_names = block_input_props
        .iter()
        .map(|(name, props)| props.get("name").unwrap_or(name));

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

    let count = get_block_defined_inputs_count(block_defined_input_props).unwrap_or(0);

    let block_defined_inputs = (0..count).map(|i| format!("{name}{i}"));

    quote! {
        inputs: vec![#(BlockPin { name: #input_field_names.to_string(), kind: HaystackKind::#input_field_kinds },)*
        #(BlockPin { name: #block_defined_inputs.to_string(), kind: HaystackKind::#kind },)*],
    }
}

/// Create the description of the outputs field
fn create_output_desc(
    block_output_props: &BTreeMap<String, BTreeMap<String, String>>,
) -> proc_macro2::TokenStream {
    let output_names = block_output_props
        .iter()
        .map(|(name, props)| props.get("name").unwrap_or(name));

    let output_kinds = block_output_props
        .iter()
        .map(|(_, props)| format_ident!("{}", props.get("kind").cloned().unwrap_or("Null".into())));

    quote! {
        outputs: vec![#(BlockPin { name: #output_names.to_string(), kind: HaystackKind::#output_kinds },)*]
    }
}

/// Ensure that the block defined inputs and user defined inputs have different names
fn ensure_unique_inputs(
    block_defined_inputs: &BTreeMap<String, String>,
    block_input_props: &BTreeMap<String, BTreeMap<String, String>>,
) {
    if let Some(count) = get_block_defined_inputs_count(block_defined_inputs) {
        (0..count).for_each(|i| {
		if block_input_props
			.keys()
			.any(|input| input == &format!("in{}", i))
		{
			eprintln!("Block defined input: 'in{}' shadows the user defined input with the same name.", i);
			panic!("Block defined inputs and user defined inputs must have different names.")
		}
	});
    }
}

/// Ensure that the block defined outputs do not shadow the user defined inputs
fn ensure_unique_outputs(
    block_defined_inputs: &BTreeMap<String, String>,
    block_output_props: &BTreeMap<String, BTreeMap<String, String>>,
) {
    if let Some(count) = get_block_defined_inputs_count(block_defined_inputs) {
        (0..count).for_each(|i| {
		if block_output_props
			.keys()
			.any(|output| output == &format!("in{}", i))
		{
			eprintln!("Block defined output: 'in{}' shadows the user defined input with the same name.", i);
			panic!("Block defined inputs and user defined outputs must have different names.")
		}
	});
    }
}

/// Get the count of block defined inputs
fn get_block_defined_inputs_count(
    block_defined_inputs: &BTreeMap<String, String>,
) -> Option<usize> {
    block_defined_inputs
        .get("count")
        .and_then(|e| e.parse::<usize>().ok())
}
