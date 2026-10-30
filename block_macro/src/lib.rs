// Copyright (c) 2022-2026, Radu Racariu.

//! Proc-macro crate for the `#[block]` attribute and `BlockProps` derive.
//!
//! Use them through the `logic-mesh` crate, which re-exports both: the
//! generated code calls into `logic_mesh::__private`, an interface only
//! the `logic-mesh` release this crate ships with provides.

extern crate proc_macro;

mod block;
mod block_props;
mod utils;

use proc_macro::TokenStream;
use syn::{DeriveInput, parse_macro_input};

/// The `block` attribute macro adds the members every block has to a
/// struct: its `id`, its `state`, and the storage for the inputs a
/// block-level `#[input(kind = "...", count = N)]` declares.
///
/// It takes no arguments. The generated code references items via
/// `::logic_mesh::...` paths by default. If the `logic-mesh` dependency
/// is renamed, override the path with the `#[logic_mesh(crate = "path")]`
/// attribute.
///
/// ```ignore
/// #[block]
/// #[derive(BlockProps, Debug)]
/// #[logic_mesh(crate = "renamed_mesh")]
/// #[category = "math"]
/// struct MyBlock { ... }
/// ```
///
/// Input and output fields are declared with the `InputImpl` and `OutputImpl`
/// types, either imported (`use logic_mesh::blocks::{InputImpl, OutputImpl};`)
/// or as fully qualified paths (`logic_mesh::blocks::InputImpl`).
#[proc_macro_attribute]
pub fn block(args: TokenStream, input: TokenStream) -> TokenStream {
    let item = proc_macro2::TokenStream::from(input.clone());
    let ast = parse_macro_input!(input as DeriveInput);

    match block::block_impl(args.into(), ast) {
        Ok(tokens) => tokens.into(),
        // Keep the item, so the error is the only one reported.
        Err(err) => {
            let err = err.into_compile_error();
            quote::quote!(#err #item).into()
        }
    }
}

/// The `BlockProps` derive generates the `BlockProps`, `BlockStaticDesc`,
/// `BlockConstruct` and `Default` impls of a block and its `new` and
/// `new_uuid` constructors, from these attributes:
///
/// - `#[category = "..."]` (required): the category the block is listed
///   under.
/// - `#[library = "..."]`: the library the block belongs to, `core` by
///   default.
/// - `#[dis = "..."]`: the name the block is displayed under, the
///   struct's name by default.
/// - `#[ver = "..."]`: the block version, `1.0.0` by default.
/// - The struct's doc comment, which becomes the block's documentation,
///   one line per `///` line.
/// - `#[input(name = "...", kind = "...")]` on an `InputImpl` field and
///   `#[output(name = "...", kind = "...")]` on an `OutputImpl` field:
///   a pin, named after the field unless `name` is given, whose `kind`
///   is a `HaystackKind` variant (`Null` by default).
/// - `#[input(kind = "...", count = N)]` on the struct: `N` more inputs,
///   named `in0`, `in1`, ... unless `name = "..."` gives another prefix.
/// - `#[logic_mesh(crate = "path")]`: the path of a renamed `logic-mesh`
///   dependency.
///
/// See the [`block`] macro docs for the `InputImpl`/`OutputImpl` field type
/// requirements.
#[proc_macro_derive(
    BlockProps,
    attributes(dis, library, category, ver, input, output, logic_mesh)
)]
pub fn block_props(input: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(input as DeriveInput);

    block_props::block_props_impl(&ast)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
