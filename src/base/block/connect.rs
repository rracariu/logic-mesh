// Copyright (c) 2022-2023, Radu Racariu.

//! Block connection trait and helpers.

use uuid::Uuid;

use super::BlockProps;
use crate::base::error::ConnectError;
use crate::base::input::InputProps;
use crate::base::link::{BaseLink, Link};
use crate::base::output::Output;

/// Block connection functions.
///
/// Implemented for every [`BlockProps`] type, trait objects included, so
/// it is never implemented by hand: bring it into scope to connect pins
/// by name.
///
/// # Errors
///
/// Each method reports a pin missing from this block as
/// [`ConnectError::OutputNotFound`] or [`ConnectError::InputNotFound`],
/// and otherwise fails as the matching free function —
/// [`connect_output`], [`disconnect_output`], [`connect_input`] or
/// [`disconnect_input`] — does.
///
/// # Examples
///
/// ```
/// use logic_mesh::{
///     ConnectError,
///     base::block::{Block, BlockConnect, BlockProps},
///     blocks::math::Add,
///     blocks::misc::SineWave,
/// };
///
/// # fn main() -> Result<(), ConnectError> {
/// let mut sine = SineWave::new();
/// let mut add = Add::new();
/// sine.connect_output("out", add.inputs_mut()[0])?;
/// sine.disconnect_output("out", add.inputs_mut()[0])?;
/// # Ok(())
/// # }
/// ```
pub trait BlockConnect: BlockProps {
    /// Connects the output named `output_name` to `target_input`,
    /// returning the new link's id.
    fn connect_output(
        &mut self,
        output_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<Uuid, ConnectError>;

    /// Connects the input named `input_name` to `target_input`,
    /// returning the new link's id.
    fn connect_input(
        &mut self,
        input_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<Uuid, ConnectError>;

    /// Disconnects the output named `source_output_name` from `target_input`.
    fn disconnect_output(
        &mut self,
        source_output_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<(), ConnectError>;

    /// Disconnects the input named `source_input_name` from `target_input`.
    fn disconnect_input(
        &mut self,
        source_input_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<(), ConnectError>;
}

/// Implements the [`BlockConnect`] trait for every type with
/// [`BlockProps`]. Being the only impl there can be, it makes the trait
/// sealed in effect: any other impl would overlap with it.
impl<T: BlockProps + ?Sized> BlockConnect for T {
    fn connect_output(
        &mut self,
        source_output_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<Uuid, ConnectError> {
        let block = *self.id();
        let mut outputs = self.outputs_mut();
        let Some(source_output) = outputs
            .iter_mut()
            .find(|output| output.desc().name == source_output_name)
        else {
            return Err(output_not_found(block, source_output_name));
        };

        connect_output(*source_output, target_input)
    }

    fn connect_input(
        &mut self,
        source_input_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<Uuid, ConnectError> {
        let block = *self.id();
        let mut inputs = self.inputs_mut();
        let Some(source_input) = inputs
            .iter_mut()
            .find(|input| input.name() == source_input_name)
        else {
            return Err(input_not_found(block, source_input_name));
        };

        connect_input(*source_input, target_input)
    }

    fn disconnect_output(
        &mut self,
        source_output_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<(), ConnectError> {
        let block = *self.id();
        let mut outputs = self.outputs_mut();
        let Some(source_output) = outputs
            .iter_mut()
            .find(|output| output.desc().name == source_output_name)
        else {
            return Err(output_not_found(block, source_output_name));
        };

        disconnect_output(*source_output, target_input)
    }

    fn disconnect_input(
        &mut self,
        source_input_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<(), ConnectError> {
        let block = *self.id();
        let mut inputs = self.inputs_mut();
        let Some(source_input) = inputs
            .iter_mut()
            .find(|input| input.name() == source_input_name)
        else {
            return Err(input_not_found(block, source_input_name));
        };

        let Some(id) = link_id_for_input(source_input.links(), target_input) else {
            return Err(not_connected(
                (source_input.block_id(), source_input.name()),
                (target_input.block_id(), target_input.name()),
            ));
        };
        source_input.remove_link_by_id(&id);
        target_input.decrement_conn();
        Ok(())
    }
}

/// Connects `source_output` to `target_input`, returning the new link's
/// id.
///
/// # Errors
///
/// Returns [`ConnectError::AlreadyConnected`] if `source_output`
/// already links to `target_input`.
///
/// # Examples
///
/// ```
/// use logic_mesh::{
///     ConnectError,
///     base::block::{Block, BlockProps, connect_output},
///     blocks::math::Add,
///     blocks::misc::SineWave,
/// };
///
/// # fn main() -> Result<(), ConnectError> {
/// let mut sine = SineWave::new();
/// let mut add = Add::new();
/// connect_output(&mut sine.out, add.inputs_mut()[0])?;
///
/// let again = connect_output(&mut sine.out, add.inputs_mut()[0]);
/// assert!(matches!(again, Err(ConnectError::AlreadyConnected { .. })));
/// # Ok(())
/// # }
/// ```
pub fn connect_output<Reader, Writer: Clone>(
    source_output: &mut dyn Output<Writer = Writer>,
    target_input: &mut dyn InputProps<Reader = Reader, Writer = Writer>,
) -> Result<Uuid, ConnectError> {
    // Connections should be unique.
    ensure_not_linked(
        &source_output.links(),
        (source_output.block_id(), source_output.name()),
        (target_input.block_id(), target_input.name()),
    )?;

    let id = Uuid::new_v4();
    let link = BaseLink::connected(
        id,
        *target_input.block_id(),
        target_input.name().to_string(),
        target_input.writer().clone(),
    );

    source_output.add_link(link);
    target_input.increment_conn();

    Ok(id)
}

/// Disconnects `source_output` from `target_input`.
///
/// # Errors
///
/// Returns [`ConnectError::NotConnected`] if `source_output` does not
/// link to `target_input`.
///
/// # Examples
///
/// ```
/// use logic_mesh::{
///     ConnectError,
///     base::block::{Block, BlockProps, connect_output, disconnect_output},
///     blocks::math::Add,
///     blocks::misc::SineWave,
/// };
///
/// # fn main() -> Result<(), ConnectError> {
/// let mut sine = SineWave::new();
/// let mut add = Add::new();
/// connect_output(&mut sine.out, add.inputs_mut()[0])?;
/// disconnect_output(&mut sine.out, add.inputs_mut()[0])?;
/// # Ok(())
/// # }
/// ```
pub fn disconnect_output<Reader, Writer: Clone>(
    source_output: &mut dyn Output<Writer = Writer>,
    target_input: &mut dyn InputProps<Reader = Reader, Writer = Writer>,
) -> Result<(), ConnectError> {
    let Some(id) = link_id_for_input(source_output.links(), target_input) else {
        return Err(not_connected(
            (source_output.block_id(), source_output.name()),
            (target_input.block_id(), target_input.name()),
        ));
    };
    source_output.remove_link_by_id(&id);
    target_input.decrement_conn();
    Ok(())
}

/// Connects `source_input` to `target_input` (input-to-input fanout),
/// returning the new link's id.
///
/// # Errors
///
/// Returns [`ConnectError::SelfLink`] if both inputs belong to the same
/// block, and [`ConnectError::AlreadyConnected`] if `source_input`
/// already links to `target_input`.
///
/// # Examples
///
/// ```
/// use logic_mesh::{
///     ConnectError,
///     base::block::{Block, BlockProps, connect_input},
///     blocks::math::{Add, Mul},
/// };
///
/// # fn main() -> Result<(), ConnectError> {
/// let mut add = Add::new();
/// let mut mul = Mul::new();
/// connect_input(add.inputs_mut()[0], mul.inputs_mut()[0])?;
/// # Ok(())
/// # }
/// ```
pub fn connect_input<Reader, Writer: Clone>(
    source_input: &mut dyn InputProps<Reader = Reader, Writer = Writer>,
    target_input: &mut dyn InputProps<Reader = Reader, Writer = Writer>,
) -> Result<Uuid, ConnectError> {
    let source = (source_input.block_id(), source_input.name());
    let target = (target_input.block_id(), target_input.name());
    ensure_not_self_link(source, target)?;
    ensure_not_linked(&source_input.links(), source, target)?;

    let id = Uuid::new_v4();
    let link = BaseLink::connected(
        id,
        *target_input.block_id(),
        target_input.name().to_string(),
        target_input.writer().clone(),
    );

    source_input.add_link(link);
    target_input.increment_conn();

    Ok(id)
}

/// Disconnects `source_input` from `target_input`.
///
/// # Errors
///
/// Returns [`ConnectError::NotConnected`] if `source_input` does not
/// link to `target_input`.
pub fn disconnect_input<I: InputProps + ?Sized>(
    source_input: &mut I,
    target_input: &mut I,
) -> Result<(), ConnectError> {
    let Some(id) = link_id_for_input(source_input.links(), target_input) else {
        return Err(not_connected(
            (source_input.block_id(), source_input.name()),
            (target_input.block_id(), target_input.name()),
        ));
    };
    source_input.remove_link_by_id(&id);
    target_input.decrement_conn();
    Ok(())
}

/// A pin at one end of a link: its block's id and its name.
pub(crate) type PinRef<'a> = (&'a Uuid, &'a str);

/// Rejects linking the input `source` to the input `target` of its own
/// block.
pub(crate) fn ensure_not_self_link(
    (source_block, source_input): PinRef<'_>,
    (target_block, target_input): PinRef<'_>,
) -> Result<(), ConnectError> {
    if source_block == target_block {
        return Err(ConnectError::SelfLink {
            block: *source_block,
            source_input: source_input.to_string(),
            target_input: target_input.to_string(),
        });
    }
    Ok(())
}

/// Rejects a second link from the pin `source`, which holds `links`, to
/// the same target input.
pub(crate) fn ensure_not_linked(
    links: &[&(dyn Link + Send)],
    (source_block, source_pin): PinRef<'_>,
    (target_block, target_input): PinRef<'_>,
) -> Result<(), ConnectError> {
    if links
        .iter()
        .any(|link| link.target_block_id() == target_block && link.target_input() == target_input)
    {
        return Err(ConnectError::AlreadyConnected {
            source_block: *source_block,
            source_pin: source_pin.to_string(),
            target_block: *target_block,
            target_input: target_input.to_string(),
        });
    }
    Ok(())
}

/// A [`ConnectError::OutputNotFound`] for `block`'s output `pin`.
pub(crate) fn output_not_found(block: Uuid, pin: &str) -> ConnectError {
    ConnectError::OutputNotFound {
        block,
        pin: pin.to_string(),
    }
}

/// A [`ConnectError::InputNotFound`] for `block`'s input `pin`.
pub(crate) fn input_not_found(block: Uuid, pin: &str) -> ConnectError {
    ConnectError::InputNotFound {
        block,
        pin: pin.to_string(),
    }
}

/// A [`ConnectError::NotConnected`] for the pin `source` and the target
/// input `target` it does not link to.
fn not_connected(
    (source_block, source_pin): PinRef<'_>,
    (target_block, target_input): PinRef<'_>,
) -> ConnectError {
    ConnectError::NotConnected {
        source_block: *source_block,
        source_pin: source_pin.to_string(),
        target_block: *target_block,
        target_input: target_input.to_string(),
    }
}

fn link_id_for_input<I: InputProps + ?Sized>(
    links: Vec<&(dyn Link + Send)>,
    target_input: &I,
) -> Option<Uuid> {
    links
        .iter()
        .find(|link| {
            link.target_input() == target_input.name()
                && link.target_block_id() == target_input.block_id()
        })
        .map(|link| *link.id())
}

#[cfg(test)]
mod test {

    use assert_matches::assert_matches;
    use libhaystack::val::kind::HaystackKind;

    use crate::base::{
        block::{Block, BlockProps},
        error::ConnectError,
        input::InputProps,
    };

    use super::{BlockConnect, connect_input};

    use crate::blocks::{InputImpl, OutputImpl};

    #[block]
    #[derive(BlockProps, Debug)]
    #[category = "test"]
    struct Block1 {
        #[input(kind = "Number")]
        input1: InputImpl,
        #[output(kind = "Number")]
        out: OutputImpl,
    }
    impl Block for Block1 {
        async fn execute(&mut self) {
            todo!()
        }
    }

    #[block]
    #[derive(BlockProps, Debug)]
    #[category = "test"]
    struct Block2 {
        #[input(kind = "Number")]
        input1: InputImpl,
        #[output(kind = "Number")]
        out: OutputImpl,
    }
    impl Block for Block2 {
        async fn execute(&mut self) {
            todo!()
        }
    }

    #[test]
    fn test_block_out_links() {
        let mut block1 = Block1::new();
        let mut block2 = Block2::new();

        assert_eq!(block1.name(), "Block1");
        assert_eq!(block2.name(), "Block2");

        let input = &mut block2.inputs_mut()[0];
        block1
            .connect_output("out", *input)
            .expect("Could not connect");

        assert!(input.is_connected());
        assert_eq!(block1.outputs()[0].links().len(), 1);

        assert!(
            block1.connect_output("out", *input).is_err(),
            "Should not be able to connect twice"
        );

        assert!(
            block1.connect_output("invalid out", *input).is_err(),
            "Should not be able to connect to invalid output"
        );

        block1
            .disconnect_output("out", *input)
            .expect("Could not disconnect");

        assert!(!input.is_connected());
        assert_eq!(block1.outputs()[0].links().len(), 0);
    }

    #[test]
    fn test_block_input_links() {
        let mut block1 = Block1::new();
        let mut block2 = Block2::new();

        let input2 = &mut block2.inputs_mut()[0];

        block1
            .connect_input("input1", *input2)
            .expect("Could not connect");

        assert!(
            block1.connect_input("input1", *input2).is_err(),
            "Should not be able to connect twice"
        );
        assert!(
            block1.connect_input("invalid input", *input2).is_err(),
            "Should not be able to connect to invalid input"
        );

        assert!(block1.input1.has_output());
        assert!(input2.is_connected());
        assert_eq!(block1.input1.links().len(), 1);
        assert_eq!(input2.links().len(), 0);

        block1
            .disconnect_input("input1", *input2)
            .expect("Could not disconnect");

        assert!(!block1.input1.is_connected());
        assert_eq!(block1.outputs()[0].links().len(), 0);
    }

    #[test]
    fn test_block_disconnect() {
        let mut block1 = Block1::new();
        let mut block2 = Block2::new();
        {
            let input2 = &mut block2.inputs_mut()[0];

            block1
                .connect_input("input1", *input2)
                .expect("Could not connect");

            assert!(
                block1.connect_input("input1", *input2).is_err(),
                "Should not be able to connect twice"
            );
            assert!(
                block1.connect_input("invalid input", *input2).is_err(),
                "Should not be able to connect to invalid input"
            );

            assert!(input2.is_connected());
            assert!(block1.input1.links().len() == 1);
        }

        block1
            .disconnect_input("input1", &mut block2.input1)
            .expect("Could not disconnect");

        assert!(!block2.input1.is_connected());
        assert!(block1.input1.links().is_empty());
    }

    /// Every failure names what failed, as a variant callers can match
    /// on, instead of a bare message.
    #[test]
    fn failures_are_typed() {
        let mut block1 = Block1::new();
        let mut block2 = Block2::new();
        let (id1, id2) = (*block1.id(), *block2.id());
        let input = block2.inputs_mut().remove(0);

        assert_matches!(
            block1.connect_output("nope", input),
            Err(ConnectError::OutputNotFound { block, pin }) if block == id1 && pin == "nope"
        );
        assert_matches!(
            block1.connect_input("nope", input),
            Err(ConnectError::InputNotFound { block, pin }) if block == id1 && pin == "nope"
        );
        // Both ends of the link are named, the source pin included.
        assert_matches!(
            block1.disconnect_output("out", input),
            Err(ConnectError::NotConnected { source_block, source_pin, target_block, target_input })
                if source_block == id1 && source_pin == "out"
                    && target_block == id2 && target_input == "input1"
        );
        assert_matches!(
            block1.disconnect_input("input1", input),
            Err(ConnectError::NotConnected { source_block, source_pin, target_block, .. })
                if source_block == id1 && source_pin == "input1" && target_block == id2
        );

        block1.connect_output("out", input).expect("linked");
        assert_matches!(
            block1.connect_output("out", input),
            Err(ConnectError::AlreadyConnected { source_block, source_pin, target_block, target_input })
                if source_block == id1 && source_pin == "out"
                    && target_block == id2 && target_input == "input1"
        );
        block1.connect_input("input1", input).expect("linked");
        assert_matches!(
            block1.connect_input("input1", input),
            Err(ConnectError::AlreadyConnected { source_pin, .. }) if source_pin == "input1"
        );

        // An input of the same block, as a block with two inputs has.
        let mut sibling = InputImpl::new("sibling", HaystackKind::Number, id1);
        assert_matches!(
            connect_input(&mut block1.input1, &mut sibling),
            Err(ConnectError::SelfLink { block, source_input, target_input })
                if block == id1 && source_input == "input1" && target_input == "sibling"
        );
    }
}
