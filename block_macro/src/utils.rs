// Copyright (c) 2022-2026, Radu Racariu.

//! Parsing of the attributes the block macros read.
//!
//! Every malformed or unknown attribute argument is reported as a
//! [`syn::Error`] spanned on the offending tokens, never a panic.

use proc_macro2::Span;
use syn::{
    Attribute, DeriveInput, Expr, ExprLit, Field, Ident, Lit, LitInt, LitStr, Meta, Type, TypePath,
    ext::IdentExt,
};

/// The block-level attributes of a block, as written.
#[derive(Default)]
pub(crate) struct BlockAttributes {
    /// `#[dis = "..."]`.
    pub(crate) dis: Option<LitStr>,
    /// `#[library = "..."]`.
    pub(crate) library: Option<LitStr>,
    /// `#[category = "..."]`.
    pub(crate) category: Option<LitStr>,
    /// `#[ver = "..."]`.
    pub(crate) ver: Option<LitStr>,
    /// The doc comment, one line per `///` line.
    pub(crate) doc: String,
}

/// Reads the block-level attributes of `ast`.
///
/// Only `dis`, `library`, `category`, `ver` and the doc comment are
/// read; any other attribute belongs to someone else and is left alone.
pub(crate) fn block_attributes(ast: &DeriveInput) -> syn::Result<BlockAttributes> {
    let mut attrs = BlockAttributes::default();
    let mut doc = Vec::new();

    for attr in &ast.attrs {
        if attr.path().is_ident("doc") {
            // `#[doc(hidden)]` and the like carry no text.
            if let Meta::NameValue(nv) = &attr.meta
                && let Expr::Lit(ExprLit {
                    lit: Lit::Str(text),
                    ..
                }) = &nv.value
            {
                doc.push(text.value());
            }
            continue;
        }

        let slot = match attr.path().get_ident() {
            Some(id) if id == "dis" => &mut attrs.dis,
            Some(id) if id == "library" => &mut attrs.library,
            Some(id) if id == "category" => &mut attrs.category,
            Some(id) if id == "ver" => &mut attrs.ver,
            _ => continue,
        };
        let value = string_attribute(attr)?;
        if slot.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                format!("duplicate `#[{}]` attribute", attr_name(attr)),
            ));
        }
        *slot = Some(value);
    }

    attrs.doc = join_doc_lines(&doc);
    Ok(attrs)
}

/// The value of a `#[name = "..."]` attribute.
fn string_attribute(attr: &Attribute) -> syn::Result<LitStr> {
    let name = attr_name(attr);
    let expected = || {
        syn::Error::new_spanned(
            attr,
            format!("expected a string value: `#[{name} = \"...\"]`"),
        )
    };
    let Meta::NameValue(nv) = &attr.meta else {
        return Err(expected());
    };
    match &nv.value {
        Expr::Lit(ExprLit {
            lit: Lit::Str(value),
            ..
        }) => Ok(value.clone()),
        _ => Err(syn::Error::new_spanned(
            &nv.value,
            format!("expected a string value: `#[{name} = \"...\"]`"),
        )),
    }
}

/// The name of an attribute whose path is a single identifier.
fn attr_name(attr: &Attribute) -> String {
    attr.path()
        .get_ident()
        .map(Ident::to_string)
        .unwrap_or_default()
}

/// Joins the lines of a doc comment with newlines, as rustdoc reads
/// them: the space after `///` is dropped, and so are the blank lines
/// before and after the text.
fn join_doc_lines(lines: &[String]) -> String {
    lines
        .iter()
        .flat_map(|text| text.split('\n'))
        .map(|line| line.strip_prefix(' ').unwrap_or(line).trim_end())
        .collect::<Vec<_>>()
        .join("\n")
        .trim_matches('\n')
        .to_string()
}

/// Resolves the crate path used in the generated code.
///
/// Defaults to `::logic_mesh`; overridden with the provided escape hatch
/// `#[logic_mesh(crate = "path")]` when the dependency is renamed.
pub(crate) fn crate_path(ast: &DeriveInput) -> syn::Result<syn::Path> {
    let mut path: Option<syn::Path> = None;
    for attr in ast.attrs.iter().filter(|a| a.path().is_ident("logic_mesh")) {
        attr.parse_nested_meta(|meta| {
            if !meta.path.is_ident("crate") {
                return Err(meta.error("unknown argument; expected `crate = \"path\"`"));
            }
            if path.is_some() {
                return Err(meta.error("duplicate `crate` argument"));
            }
            let lit: LitStr = meta.value()?.parse()?;
            path = Some(lit.parse()?);
            Ok(())
        })?;
    }

    Ok(path.unwrap_or_else(|| syn::parse_quote!(::logic_mesh)))
}

/// Which kind of pin a field or an attribute declares.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PinType {
    /// An `InputImpl` field marked `#[input]`.
    Input,
    /// An `OutputImpl` field marked `#[output]`.
    Output,
}

impl PinType {
    /// The helper attribute that declares the pin.
    fn attribute(self) -> &'static str {
        match self {
            PinType::Input => "input",
            PinType::Output => "output",
        }
    }

    /// The name of the type a pin field has.
    fn field_type(self) -> &'static str {
        match self {
            PinType::Input => "InputImpl",
            PinType::Output => "OutputImpl",
        }
    }
}

/// A pin declared by a field.
pub(crate) struct FieldPin {
    /// The field holding the pin.
    pub(crate) field: Ident,
    /// The pin name: `name = "..."`, or the field's name.
    pub(crate) name: LitStr,
    /// The pin's `HaystackKind` variant, spanned on `kind = "..."`.
    pub(crate) kind: Ident,
}

/// The inputs the block-level `#[input(kind = "...", count = N)]`
/// attribute declares: `count` inputs named `name0`, `name1`, ...
pub(crate) struct BlockInputs {
    /// The attribute, for spanning errors.
    pub(crate) span: Span,
    /// The name prefix, `in` unless `name = "..."` is given.
    pub(crate) name: String,
    /// The inputs' `HaystackKind` variant.
    pub(crate) kind: Ident,
    /// How many inputs there are.
    pub(crate) count: usize,
}

impl BlockInputs {
    /// The names of the inputs, in order.
    pub(crate) fn names(&self) -> Vec<String> {
        (0..self.count)
            .map(|i| format!("{}{i}", self.name))
            .collect()
    }
}

/// The arguments of one pin attribute.
#[derive(Default)]
struct PinArgs {
    name: Option<LitStr>,
    kind: Option<LitStr>,
    count: Option<LitInt>,
}

/// Parses `#[input(...)]` / `#[output(...)]`, accepting only `allowed`
/// arguments. A bare `#[input]` takes every default.
fn pin_args(attr: &Attribute, allowed: &[&str]) -> syn::Result<PinArgs> {
    let mut args = PinArgs::default();
    if let Meta::Path(_) = attr.meta {
        return Ok(args);
    }
    let attr_name = attr_name(attr);
    attr.parse_nested_meta(|meta| {
        let Some(arg) = meta.path.get_ident().map(Ident::to_string) else {
            return Err(meta.error("expected an argument name"));
        };
        if !allowed.contains(&arg.as_str()) {
            let expected = allowed
                .iter()
                .map(|arg| format!("`{arg}`"))
                .collect::<Vec<_>>()
                .join(", ");
            return Err(meta.error(format!(
                "unknown `#[{attr_name}(...)]` argument `{arg}`; expected one of {expected}"
            )));
        }
        let duplicate = match arg.as_str() {
            "name" => args.name.replace(meta.value()?.parse()?).is_some(),
            "kind" => args.kind.replace(meta.value()?.parse()?).is_some(),
            _ => args.count.replace(meta.value()?.parse()?).is_some(),
        };
        if duplicate {
            return Err(meta.error(format!("duplicate `{arg}` argument")));
        }
        Ok(())
    })?;
    Ok(args)
}

/// The `HaystackKind` variant `kind = "..."` names, `Null` if none.
///
/// The identifier is spanned on the literal, so a compiler error about
/// the variant points there. A raw identifier (`r#Number`) is rejected:
/// the value is a variant's name, and no variant needs the raw form.
fn kind_ident(kind: Option<&LitStr>) -> syn::Result<Ident> {
    let Some(kind) = kind else {
        return Ok(Ident::new("Null", Span::call_site()));
    };
    let expected = || {
        syn::Error::new(
            kind.span(),
            "expected the name of a `HaystackKind` variant, such as \"Number\"",
        )
    };
    // `LitStr::parse` spans what it parses on the literal.
    let ident: Ident = kind.parse().map_err(|_| expected())?;
    if ident.unraw() != ident {
        return Err(expected());
    }
    Ok(ident)
}

/// Reads the block-level `#[input(...)]` attribute, if there is one.
pub(crate) fn block_inputs(ast: &DeriveInput) -> syn::Result<Option<BlockInputs>> {
    let mut found: Option<BlockInputs> = None;
    for attr in &ast.attrs {
        if attr.path().is_ident("output") {
            return Err(syn::Error::new_spanned(
                attr,
                "`#[output]` goes on an `OutputImpl` field",
            ));
        }
        if !attr.path().is_ident("input") {
            continue;
        }
        if found.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "duplicate block-level `#[input(...)]` attribute",
            ));
        }
        let args = pin_args(attr, &["name", "kind", "count"])?;
        let Some(count) = args.count else {
            return Err(syn::Error::new_spanned(
                attr,
                "a block-level `#[input(...)]` needs `count = N`, the number of inputs",
            ));
        };
        found = Some(BlockInputs {
            span: syn::spanned::Spanned::span(attr),
            name: args
                .name
                .map(|name| name.value())
                .unwrap_or_else(|| "in".to_string()),
            kind: kind_ident(args.kind.as_ref())?,
            count: count.base10_parse()?,
        });
    }
    Ok(found)
}

/// Whether a block-level `#[input]` attribute is present, malformed or
/// not; the derive reports what is wrong with it.
pub(crate) fn has_block_inputs(ast: &DeriveInput) -> bool {
    ast.attrs.iter().any(|attr| attr.path().is_ident("input"))
}

/// The named fields of the struct `ast`, or an error if it is not a
/// struct with named fields.
pub(crate) fn named_fields<'a>(
    ast: &'a DeriveInput,
    macro_name: &str,
) -> syn::Result<impl Iterator<Item = &'a Field>> {
    match &ast.data {
        syn::Data::Struct(syn::DataStruct {
            fields: syn::Fields::Named(fields),
            ..
        }) => Ok(fields.named.iter()),
        _ => Err(syn::Error::new_spanned(
            &ast.ident,
            format!("`{macro_name}` applies to structs with named fields"),
        )),
    }
}

/// Reads the pins of type `pin_type` the fields of `ast` declare.
///
/// A pin is a field of the pin's type (matched on the last path
/// segment, so `InputImpl` and `logic_mesh::blocks::InputImpl` both
/// are) marked with the pin's attribute. A field of the pin's type
/// without it is not a pin; the attribute on any other field is an
/// error.
pub(crate) fn field_pins(ast: &DeriveInput, pin_type: PinType) -> syn::Result<Vec<FieldPin>> {
    let mut pins = Vec::new();
    for field in named_fields(ast, "BlockProps")? {
        let mut attrs = field
            .attrs
            .iter()
            .filter(|attr| attr.path().is_ident(pin_type.attribute()));
        let Some(attr) = attrs.next() else {
            continue;
        };
        if let Some(duplicate) = attrs.next() {
            return Err(syn::Error::new_spanned(
                duplicate,
                format!("duplicate `#[{}]` attribute", pin_type.attribute()),
            ));
        }
        if !type_is(&field.ty, pin_type.field_type()) {
            return Err(syn::Error::new_spanned(
                attr,
                format!(
                    "`#[{}]` goes on an `{}` field",
                    pin_type.attribute(),
                    pin_type.field_type()
                ),
            ));
        }
        let Some(field_ident) = &field.ident else {
            continue;
        };

        let args = pin_args(attr, &["name", "kind"])?;
        pins.push(FieldPin {
            field: field_ident.clone(),
            // A field named with a keyword, `r#in`, names the pin `in`.
            name: args.name.unwrap_or_else(|| {
                LitStr::new(&field_ident.unraw().to_string(), field_ident.span())
            }),
            kind: kind_ident(args.kind.as_ref())?,
        });
    }
    Ok(pins)
}

/// Whether `ty` is a path whose last segment is `name`, so unrelated
/// types that merely share the prefix (e.g. `InputImplConfig`) are not
/// taken for pins.
fn type_is(ty: &Type, name: &str) -> bool {
    matches!(ty, Type::Path(TypePath { path, .. })
        if path.segments.last().is_some_and(|seg| seg.ident == name))
}
