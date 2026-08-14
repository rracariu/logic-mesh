// Copyright (c) 2022-2023, Radu Racariu.

//! Proc-macro crate for the `#[block]` attribute and `BlockProps` derive.

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

/// The `block` attribute macro derives the `Block` trait for a struct.
///
/// The generated code references items via `::logic_mesh::...` paths by
/// default. If the `logic-mesh` dependency is renamed, override the path
/// with the `#[logic_mesh(crate = "path")]` attribute.
///
/// ```ignore
/// #[block]
/// #[derive(BlockProps, Debug)]
/// #[logic_mesh(crate = "renamed_mesh")]
/// struct MyBlock { ... }
/// ```
///
/// Input and output fields are declared with the `InputImpl` and `OutputImpl`
/// types, either imported (`use logic_mesh::blocks::{InputImpl, OutputImpl};`)
/// or as fully qualified paths (`logic_mesh::blocks::InputImpl`).
#[allow(clippy::let_and_return)]
#[proc_macro_attribute]
pub fn block(_args: TokenStream, input: TokenStream) -> TokenStream {
    let gen = block_impl(input);

    //eprintln!("block: {gen}");

    gen
}

/// The `block_props` derive macro generates the `BlockProps` trait for a struct.
///
/// See the [`block`] macro docs for the `InputImpl`/`OutputImpl` field type
/// requirements and the `#[logic_mesh(crate = "path")]` override for renamed
/// dependencies.
#[allow(clippy::let_and_return)]
#[proc_macro_derive(
    BlockProps,
    attributes(dis, library, category, input, output, logic_mesh)
)]
pub fn block_props(input: TokenStream) -> TokenStream {
    let ast: syn::DeriveInput = syn::parse(input).unwrap();

    let gen = block_props_impl(&ast);

    //eprintln!("block_props: {gen}");

    gen
}
