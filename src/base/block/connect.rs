// Copyright (c) 2022-2023, Radu Racariu.

//!
//! Defines the block connection trait
//!

use uuid::Uuid;

use super::desc::BlockStaticDesc;
use super::{Block, BlockProps};
use crate::base::input::InputProps;
use crate::base::link::{BaseLink, Link, LinkState};
use crate::base::output::Output;

/// Block connection functions
pub trait BlockConnect: BlockStaticDesc {
    /// Connect a block output to the given input
    ///
    /// # Arguments
    /// - output_name: The name of the output to be connected
    /// - input: The block input to be connected
    ///
    fn connect_output(
        &mut self,
        output_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<Uuid, &'static str>;

    /// Connect a block input to another's block input
    ///
    /// # Arguments
    /// - input_name: The name of the output to be connected
    /// - input: The block input to be connected
    ///
    fn connect_input(
        &mut self,
        input_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<Uuid, &'static str>;

    /// Disconnect a block output from the given input
    /// # Arguments
    /// - source_output_name: The name of the source output to be disconnected
    /// - target_input: The target input to be disconnected
    ///
    fn disconnect_output(
        &mut self,
        source_output_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<(), &'static str>;

    /// Disconnect a block input from the given output
    /// # Arguments
    /// - source_input_name: The name of the source input to be disconnected
    /// - target_input: The target input to be disconnected
    fn disconnect_input(
        &mut self,
        source_input_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<(), &'static str>;
}

///
/// Implements the `BlockConnect` trait for all types
/// that are `Block`s
///
impl<T: Block + ?Sized> BlockConnect for T {
    fn connect_output(
        &mut self,
        source_output_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<Uuid, &'static str> {
        let mut outputs = self.outputs_mut();
        let source_output = if let Some(out) = outputs
            .iter_mut()
            .find(|output| output.desc().name == source_output_name)
        {
            out
        } else {
            return Err("Output not found");
        };

        connect_output(*source_output, target_input)
    }

    fn connect_input(
        &mut self,
        source_input_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<Uuid, &'static str> {
        let mut inputs = self.inputs_mut();
        let source_input = if let Some(input) = inputs
            .iter_mut()
            .find(|input| input.name() == source_input_name)
        {
            *input as &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>
        } else {
            return Err("Input not found");
        };
        connect_input(source_input, target_input)
    }

    fn disconnect_output(
        &mut self,
        source_output_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<(), &'static str> {
        let mut outputs = self.outputs_mut();
        let source_output = if let Some(out) = outputs
            .iter_mut()
            .find(|output| output.desc().name == source_output_name)
        {
            out
        } else {
            return Err("Output not found");
        };

        disconnect_output(*source_output, target_input)
    }

    fn disconnect_input(
        &mut self,
        source_input_name: &str,
        target_input: &mut dyn InputProps<Reader = Self::Reader, Writer = Self::Writer>,
    ) -> Result<(), &'static str> {
        let mut inputs = self.inputs_mut();
        let source_input = if let Some(in_) = inputs
            .iter_mut()
            .find(|input| input.name() == source_input_name)
        {
            in_
        } else {
            return Err("Input not found");
        };

        let link_id = link_id_for_input(source_input.links(), target_input);

        if let Some(id) = link_id {
            source_input.remove_link_by_id(&id);
            target_input.decrement_conn();
            Ok(())
        } else {
            Err("No connection found")
        }
    }
}

/// Connect a block output to the given input
/// # Arguments
/// - source_output: The output to be connected
/// - target_input: The block input to be connected
pub fn connect_output<Reader, Writer: Clone>(
    source_output: &mut dyn Output<Writer = Writer>,
    target_input: &mut dyn InputProps<Reader = Reader, Writer = Writer>,
) -> Result<Uuid, &'static str> {
    // Connections should be unique.
    if source_output.links().iter().any(|link| {
        link.target_block_id() == target_input.block_id()
            && link.target_input() == target_input.name()
    }) {
        return Err("Already connected");
    }

    let mut link = BaseLink::new(*target_input.block_id(), target_input.name().to_string());
    let id = link.id;

    link.tx = Some(target_input.writer().clone());

    link.state = LinkState::Connected;

    source_output.add_link(link);
    target_input.increment_conn();

    Ok(id)
}

/// Disconnect a block output from the given input
/// # Arguments
/// - source_output: The output to be disconnected
/// - target_input: The block input to be disconnected
///
/// # Returns
/// - `Ok(())` if the disconnection was successful, `Err` otherwise
pub fn disconnect_output<Reader, Writer: Clone>(
    source_output: &mut dyn Output<Writer = Writer>,
    target_input: &mut dyn InputProps<Reader = Reader, Writer = Writer>,
) -> Result<(), &'static str> {
    let link_id = link_id_for_input(source_output.links(), target_input);

    if let Some(id) = link_id {
        source_output.remove_link_by_id(&id);
        target_input.decrement_conn();
        Ok(())
    } else {
        Err("No connection found")
    }
}

/// Connect a block input to another's block input
/// # Arguments
/// - source_input: The input to be connected to
/// - target_input: The block input to be connected
pub fn connect_input<Reader, Writer: Clone>(
    source_input: &mut dyn InputProps<Reader = Reader, Writer = Writer>,
    target_input: &mut dyn InputProps<Reader = Reader, Writer = Writer>,
) -> Result<Uuid, &'static str> {
    if source_input.block_id() == target_input.block_id() {
        return Err("Cannot connect to the same block");
    }

    if source_input.links().iter().any(|link| {
        link.target_block_id() == target_input.block_id()
            && link.target_input() == target_input.name()
    }) {
        return Err("Already connected");
    }

    let mut link = BaseLink::new(*target_input.block_id(), target_input.name().to_string());
    let id = link.id;

    link.tx = Some(target_input.writer().clone());

    link.state = LinkState::Connected;

    source_input.add_link(link);
    target_input.increment_conn();

    Ok(id)
}

/// Disconnect a block input from another's block input
/// # Arguments
/// - source_input: The input to be disconnected from
/// - target_input: The block input to be disconnected
/// # Returns
/// - `Ok(())` if the disconnection was successful, `Err` otherwise
pub fn disconnect_input<I: InputProps + ?Sized>(
    source_input: &mut I,
    target_input: &mut I,
) -> Result<(), &'static str> {
    let link_id = link_id_for_input(source_input.links(), target_input);

    if let Some(id) = link_id {
        source_input.remove_link_by_id(&id);
        target_input.decrement_conn();
        Ok(())
    } else {
        Err("No connection found")
    }
}

/// Disconnect all the inputs and outputs of a block
/// # Arguments
/// - block: The block to be disconnected
/// - decrement_target_input: A function that decrements the number of connections of a block input
pub fn disconnect_block<B, F>(block: &mut B, mut decrement_target_input: F)
where
    B: BlockProps + ?Sized,
    F: FnMut(&Uuid, &str) -> Option<usize>,
{
    block
        .outputs_mut()
        .iter()
        .filter(|output| output.is_connected())
        .for_each(|out| {
            out.links().iter_mut().for_each(|link| {
                decrement_target_input(link.target_block_id(), link.target_input());
            });
        });

    block
        .inputs_mut()
        .iter()
        .filter(|input| input.has_output())
        .for_each(|src_input| {
            src_input.links().iter_mut().for_each(|link| {
                decrement_target_input(link.target_block_id(), link.target_input());
            });
        });

    block.remove_all_links();
}

/// Disconnect a link from a block
/// # Arguments
/// - block: The block to be disconnected
/// - link_id: The id of the link to be disconnected
/// - decrement_target_input: A function that decrements the number of connections of a block input
pub fn disconnect_link<B, F>(block: &mut B, link_id: &Uuid, mut decrement_target_input: F) -> bool
where
    B: BlockProps + ?Sized,
    F: FnMut(&Uuid, &str) -> Option<usize>,
{
    if !block
        .outputs_mut()
        .iter()
        .filter(|output| output.is_connected())
        .any(|out| {
            out.links().iter_mut().any(|link| {
                if link.id() == link_id {
                    decrement_target_input(link.target_block_id(), link.target_input());
                    true
                } else {
                    false
                }
            })
        })
        && !block
            .inputs_mut()
            .iter()
            .filter(|input| input.has_output())
            .any(|src_input| {
                src_input.links().iter_mut().any(|link| {
                    if link.id() == link_id {
                        decrement_target_input(link.target_block_id(), link.target_input());
                        true
                    } else {
                        false
                    }
                })
            })
    {
        return false;
    }

    block.remove_link_by_id(link_id);
    true
}

fn link_id_for_input<I: InputProps + ?Sized>(
    links: Vec<&dyn Link>,
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

    use uuid::Uuid;

    use crate::base::{
        block::{Block, BlockDesc, BlockProps, BlockState, connect::disconnect_block},
        input::{Input, InputProps},
        output::Output,
    };

    use super::BlockConnect;

    use crate::base::block::test_utils::mock::{InputImpl, OutputImpl};

    use libhaystack::val::kind::HaystackKind;

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

        let gid = block2.id().clone();
        let input1 = &mut block2.input1;

        disconnect_block(&mut block1, |id, name| {
            assert!(*id == gid);
            assert!(name == "input1");
            Some(input1.decrement_conn())
        });

        assert!(!input1.is_connected());
        assert!(block1.input1.links().len() == 0);
    }
}
