// Copyright (c) 2022-2023, Radu Racariu.

//! The seal on the pin and link traits.

/// Private supertrait of [`InputProps`](crate::base::input::InputProps),
/// [`OutputProps`](crate::base::output::OutputProps) and
/// [`Link`](crate::base::link::Link), and through them of
/// [`Input`](crate::base::input::Input) and
/// [`Output`](crate::base::output::Output).
///
/// The module is private to the crate, so no other crate can name this
/// trait or implement it. Only the crate's pin and link types can then
/// implement those traits, and a method can be added to them without a
/// major release.
pub trait Sealed {}
