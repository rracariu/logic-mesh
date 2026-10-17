// Copyright (c) 2022-2026, Radu Racariu.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse::Parser};

use crate::utils::{crate_path, has_block_inputs};

/// Implements the `block` attribute macro.
///
/// Adds the `id` and `state` members to the struct, and the `_inputs`
/// member if the struct declares block-level inputs.
///
/// The attribute takes no arguments: a renamed crate is named with
/// `#[logic_mesh(crate = "path")]`, which the `BlockProps` derive reads
/// too. A rejected argument is reported next to the expanded struct, so
/// it is the only error the user sees.
pub(super) fn block_impl(args: TokenStream, mut ast: DeriveInput) -> syn::Result<TokenStream> {
    let args_error = (!args.is_empty()).then(|| {
        syn::Error::new_spanned(
            &args,
            "`#[block]` takes no arguments; name a renamed `logic-mesh` crate with \
             `#[logic_mesh(crate = \"path\")]`",
        )
        .into_compile_error()
    });

    // An invalid `#[logic_mesh(...)]` is reported by the derive, which
    // reads it too; reporting it here as well would repeat the error.
    let krate = crate_path(&ast).unwrap_or_else(|_| syn::parse_quote!(::logic_mesh));
    let block_inputs = has_block_inputs(&ast);

    let syn::Data::Struct(syn::DataStruct {
        fields: syn::Fields::Named(fields),
        ..
    }) = &mut ast.data
    else {
        return Err(syn::Error::new_spanned(
            &ast.ident,
            "`#[block]` applies to structs with named fields",
        ));
    };

    let parse = syn::Field::parse_named;
    fields
        .named
        .push(parse.parse2(quote! { #[doc = "Block UUID."] id: #krate::Uuid })?);
    fields.named.push(parse.parse2(quote! {
        #[doc = "Current block state."] state: #krate::base::block::BlockState
    })?);
    if block_inputs {
        fields.named.push(parse.parse2(quote! {
            #[doc = "Auto-generated input pins."]
            _inputs: ::std::vec::Vec<#krate::blocks::InputImpl>
        })?);
    }

    Ok(quote! {
        #args_error
        #[allow(missing_docs)]
        #ast
    })
}
