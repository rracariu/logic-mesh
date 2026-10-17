// Copyright (c) 2022-2026, Radu Racariu.

use std::collections::BTreeSet;

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{DeriveInput, Ident, LitStr};

use crate::utils::{
    BlockInputs, FieldPin, PinType, block_attributes, block_inputs, crate_path, field_pins,
    named_fields,
};

/// Fields the `block` attribute adds, which the constructor sets itself.
const BLOCK_FIELDS: [&str; 3] = ["id", "state", "_inputs"];

/// Generates the implementation for the `BlockProps` trait and creates the constructor function.
///
/// It also initializes any block fields that are not inputs or output
/// to their default type value.
///
/// The generated code reaches the crate only through public traits and
/// the `__private` module, never through a struct literal or a public
/// constructor, and names everything else by its full `::core` /
/// `::std` path, so it compiles whatever the user's module imports.
pub(super) fn block_props_impl(ast: &DeriveInput) -> syn::Result<TokenStream> {
    let block_ident = &ast.ident;
    // Report a struct without named fields before anything it lacks.
    let _ = named_fields(ast, "BlockProps")?;
    let krate = crate_path(ast)?;
    let krate = &krate;

    let attrs = block_attributes(ast)?;
    let Some(category) = attrs.category else {
        return Err(syn::Error::new_spanned(
            block_ident,
            format!(
                "block `{block_ident}` needs a category: add `#[category = \"...\"]`, \
                 the category the block is listed under"
            ),
        ));
    };
    let name = block_ident.to_string();
    let dis = attrs
        .dis
        .unwrap_or_else(|| LitStr::new(&name, block_ident.span()));
    let library = attrs
        .library
        .unwrap_or_else(|| LitStr::new("core", Span::call_site()));
    let ver = attrs
        .ver
        .unwrap_or_else(|| LitStr::new("1.0.0", Span::call_site()));
    let doc = attrs.doc;

    let block_inputs = block_inputs(ast)?;
    let inputs = field_pins(ast, PinType::Input)?;
    let outputs = field_pins(ast, PinType::Output)?;
    if outputs.is_empty() {
        return Err(syn::Error::new_spanned(
            block_ident,
            format!(
                "block `{block_ident}` needs an output: an `OutputImpl` field marked `#[output]`"
            ),
        ));
    }
    ensure_unique_pin_names(block_inputs.as_ref(), &inputs, &outputs)?;

    // Pins as the description lists them: the input fields first, then
    // the block-level inputs.
    let block_input_names = block_inputs
        .as_ref()
        .map(BlockInputs::names)
        .unwrap_or_default();
    let block_input_kind = block_inputs
        .as_ref()
        .map(|inputs| inputs.kind.clone())
        .unwrap_or_else(|| Ident::new("Null", Span::call_site()));
    let input_names = inputs.iter().map(|pin| &pin.name);
    let input_kinds = inputs.iter().map(|pin| &pin.kind);
    let output_names = outputs.iter().map(|pin| &pin.name);
    let output_kinds = outputs.iter().map(|pin| &pin.kind);

    // Constructor field initializers.
    let input_init = pin_inits(&inputs, krate, "input");
    let output_init = pin_inits(&outputs, krate, "output");
    let block_inputs_init = block_inputs.as_ref().map(|_| {
        let names = &block_input_names;
        quote! {
            _inputs: ::std::vec![
                #(#krate::__private::input(#names, #krate::HaystackKind::#block_input_kind, uuid),)*
            ],
        }
    });
    let other_fields = other_fields(ast, &inputs, &outputs)?;
    let other_init = other_fields.iter().map(|(field, ty)| {
        quote! { #field: <#ty as ::core::default::Default>::default(), }
    });

    // Pin accessors.
    let input_dyn = quote!(dyn #krate::base::block::BlockInput<Self::Reader, Self::Writer> + ::core::marker::Send);
    let output_dyn =
        quote!(dyn #krate::base::block::BlockOutput<Self::Writer> + ::core::marker::Send);
    let input_fields: Vec<_> = inputs.iter().map(|pin| &pin.field).collect();
    let output_fields: Vec<_> = outputs.iter().map(|pin| &pin.field).collect();
    let (block_inputs_ref, block_inputs_mut) = match block_inputs {
        Some(_) => (
            quote! { inputs.extend(self._inputs.iter().map(|input| input as &(#input_dyn))); },
            quote! { inputs.extend(self._inputs.iter_mut().map(|input| input as &mut (#input_dyn))); },
        ),
        None => (TokenStream::new(), TokenStream::new()),
    };

    let props = quote!(#krate::base::block::BlockProps);
    let input_props = quote!(#krate::base::input::InputProps);
    let output_props = quote!(#krate::base::output::OutputProps);

    let new_doc = format!("Creates a new [`{block_ident}`].");
    let new_uuid_doc = format!("Creates a new [`{block_ident}`] with the given UUID.");

    Ok(quote! {
        // Generated constructors
        impl #block_ident {
            #[doc = #new_doc]
            pub fn new() -> Self {
                Self::new_uuid(#krate::Uuid::new_v4())
            }

            #[doc = #new_uuid_doc]
            pub fn new_uuid(uuid: #krate::Uuid) -> Self {
                Self {
                    id: uuid,
                    state: #krate::base::block::BlockState::Running,
                    #(#other_init)*
                    #output_init
                    #input_init
                    #block_inputs_init
                }
            }
        }

        // Implementation of the BlockProps trait
        // using the attributes
        impl #props for #block_ident {
            type Reader = <#krate::blocks::InputImpl as #input_props>::Reader;
            type Writer = <#krate::blocks::InputImpl as #input_props>::Writer;

            fn id(&self) -> &#krate::Uuid {
                &self.id
            }

            fn name(&self) -> &str {
                &<Self as #props>::desc(self).name
            }

            fn desc(&self) -> &#krate::base::block::BlockDesc {
                <Self as #krate::base::block::BlockStaticDesc>::static_desc()
            }

            fn state(&self) -> #krate::base::block::BlockState {
                ::core::clone::Clone::clone(&self.state)
            }

            fn set_state(&mut self, state: #krate::base::block::BlockState) -> #krate::base::block::BlockState {
                self.state = state;
                ::core::clone::Clone::clone(&self.state)
            }

            fn inputs(&self) -> ::std::vec::Vec<&(#input_dyn)> {
                #[allow(unused_mut)]
                let mut inputs: ::std::vec::Vec<&(#input_dyn)> =
                    ::std::vec![#(&self.#input_fields as &(#input_dyn),)*];
                #block_inputs_ref
                inputs
            }

            fn inputs_mut(&mut self) -> ::std::vec::Vec<&mut (#input_dyn)> {
                #[allow(unused_mut)]
                let mut inputs: ::std::vec::Vec<&mut (#input_dyn)> =
                    ::std::vec![#(&mut self.#input_fields as &mut (#input_dyn),)*];
                #block_inputs_mut
                inputs
            }

            fn outputs(&self) -> ::std::vec::Vec<&(#output_dyn)> {
                ::std::vec![#(&self.#output_fields as &(#output_dyn),)*]
            }

            fn outputs_mut(&mut self) -> ::std::vec::Vec<&mut (#output_dyn)> {
                ::std::vec![#(&mut self.#output_fields as &mut (#output_dyn),)*]
            }

            fn links(&self) -> ::std::vec::Vec<(&str, ::std::vec::Vec<&(dyn #krate::base::link::Link + ::core::marker::Send)>)> {
                let mut links = ::std::vec::Vec::new();
                for input in <Self as #props>::inputs(self) {
                    links.push((#input_props::name(input), #input_props::links(input)));
                }
                for output in <Self as #props>::outputs(self) {
                    links.push((#output_props::name(output), #output_props::links(output)));
                }
                links
            }

            fn remove_link_by_id(&mut self, link_uuid: &#krate::Uuid) {
                for input in <Self as #props>::inputs_mut(self) {
                    #input_props::remove_link_by_id(input, link_uuid);
                }
                for output in <Self as #props>::outputs_mut(self) {
                    #output_props::remove_link_by_id(output, link_uuid);
                }
            }

            fn remove_all_links(&mut self) {
                for input in <Self as #props>::inputs_mut(self) {
                    #input_props::remove_all_links(input);
                }
                for output in <Self as #props>::outputs_mut(self) {
                    #output_props::remove_all_links(output);
                }
            }
        }

        // Implementation of the BlockStaticDesc trait
        // using the attributes
        impl #krate::base::block::BlockStaticDesc for #block_ident {
            fn static_desc() -> &'static #krate::base::block::BlockDesc {
                static DESC: ::std::sync::LazyLock<#krate::base::block::BlockDesc> =
                    ::std::sync::LazyLock::new(|| {
                        #krate::__private::native_desc(#krate::__private::NativeDesc {
                            name: #name,
                            library: #library,
                            dis: #dis,
                            category: #category,
                            ver: #ver,
                            doc: #doc,
                            inputs: &[
                                #((#input_names, #krate::HaystackKind::#input_kinds),)*
                                #((#block_input_names, #krate::HaystackKind::#block_input_kind),)*
                            ],
                            outputs: &[#((#output_names, #krate::HaystackKind::#output_kinds),)*],
                        })
                    });

                &DESC
            }
        }

        impl ::core::default::Default for #block_ident {
            fn default() -> Self {
                Self::new()
            }
        }

        impl #krate::base::block::BlockConstruct for #block_ident {
            fn with_uuid(uuid: #krate::Uuid) -> Self {
                Self::new_uuid(uuid)
            }
        }
    })
}

/// Initializers of the pin fields `pins`, each built by the
/// `__private` function `constructor`.
fn pin_inits(pins: &[FieldPin], krate: &syn::Path, constructor: &str) -> TokenStream {
    let constructor = Ident::new(constructor, Span::call_site());
    let fields = pins.iter().map(|pin| &pin.field);
    let names = pins.iter().map(|pin| &pin.name);
    let kinds = pins.iter().map(|pin| &pin.kind);
    quote! {
        #(#fields: #krate::__private::#constructor(#names, #krate::HaystackKind::#kinds, uuid),)*
    }
}

/// The fields that are neither pins nor added by the `block`
/// attribute, which the constructor sets to their default value.
fn other_fields<'a>(
    ast: &'a DeriveInput,
    inputs: &[FieldPin],
    outputs: &[FieldPin],
) -> syn::Result<Vec<(&'a Ident, &'a syn::Type)>> {
    let is_pin = |ident: &Ident| inputs.iter().chain(outputs).any(|pin| &pin.field == ident);
    Ok(named_fields(ast, "BlockProps")?
        .filter_map(|field| field.ident.as_ref().map(|ident| (ident, &field.ty)))
        .filter(|(ident, _)| !BLOCK_FIELDS.iter().any(|name| ident == name) && !is_pin(ident))
        .collect())
}

/// Ensures no two pins of the block share a name. An engine looks a
/// link's source pin up by name among the outputs and then the inputs,
/// so a name can only be used once on a block.
fn ensure_unique_pin_names(
    block_inputs: Option<&BlockInputs>,
    inputs: &[FieldPin],
    outputs: &[FieldPin],
) -> syn::Result<()> {
    let mut seen = BTreeSet::new();
    // The block-level inputs go first, so a clash is reported on the
    // field pin's own name.
    let block_pins = block_inputs
        .map(|inputs| {
            inputs
                .names()
                .into_iter()
                .map(|name| (name, inputs.span))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let pins = inputs
        .iter()
        .chain(outputs)
        .map(|pin| (pin.name.value(), pin.name.span()));

    for (name, span) in block_pins.into_iter().chain(pins) {
        if !seen.insert(name.clone()) {
            return Err(syn::Error::new(
                span,
                format!("pin name `{name}` is used by another pin of this block"),
            ));
        }
    }
    Ok(())
}
