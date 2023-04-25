// Copyright (c) 2022-2023, IntriSemantics Corp.

extern crate proc_macro;
extern crate proc_macro2;
extern crate syn;
#[macro_use]
extern crate quote;

mod block;
mod block_props;
mod utils;

use block::block_impl;
use block_props::block_props_impl;
use proc_macro::TokenStream;

/// The `block` attribute macro
/// This macro is used to derive the `Block` trait for a struct
#[allow(clippy::let_and_return)]
#[proc_macro_attribute]
pub fn block(_args: TokenStream, input: TokenStream) -> TokenStream {
    let gen = block_impl(input);

    //eprintln!("block: {gen}");

    gen
}

/// The `block_props` attribute macro
/// This macro is used to derive the `BlockProps` trait for a struct
#[allow(clippy::let_and_return)]
#[proc_macro_derive(BlockProps, attributes(name, library, category, input, output))]
pub fn block_props(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();

    let gen = block_props_impl(&ast);

    //eprintln!("block_props: {gen}");

    gen
}
