// Copyright (c) 2022-2023, IntriSemantics Corp.

//!
//! Defines the block connection trait
//!

use uuid::Uuid;

use super::desc::BlockStaticDesc;
use super::Block;
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
    fn connect_output<I: InputProps<Writer = Self::Writer> + ?Sized>(
        &mut self,
        output_name: &str,
        target_input: &mut I,
    ) -> Result<(), &'static str>;

    /// Connect a block input to another's block input
    ///
    /// # Arguments
    /// - output_name: The name of the output to be connected
    /// - input: The block input to be connected
    ///
    fn connect_input<I: InputProps<Writer = Self::Writer> + ?Sized>(
        &mut self,
        source_input: &mut I,
        target_input: &mut I,
    ) -> Result<(), &'static str>;

    /// Disconnect a block output from the given input
    /// # Arguments
    /// - input: The block input to be disconnected
    ///
    fn disconnect_output<I: InputProps<Writer = Self::Writer>>(
        &mut self,
        output_name: &str,
        input: &mut I,
    ) -> Result<(), &'static str>;

    /// Disconnect a block input from the given output
    /// # Arguments
    /// - input_name: The name of the input to be disconnected
    /// - input: The block input to be disconnected
    fn disconnect_input<I: InputProps>(
        &mut self,
        input_name: &str,
        input: &mut I,
    ) -> Result<(), &'static str>;
}

///
/// Implements the `BlockConnect` trait for all types
/// that are `Block`s
///
impl<T: Block> BlockConnect for T {
    fn connect_output<I: InputProps<Writer = Self::Writer> + ?Sized>(
        &mut self,
        output_name: &str,
        target_input: &mut I,
    ) -> Result<(), &'static str> {
        let mut outputs = self.outputs_mut();
        let source_output = if let Some(out) = outputs
            .iter_mut()
            .find(|output| output.desc().name == output_name)
        {
            out
        } else {
            return Err("Output not found");
        };

        connect_output(*source_output, target_input)
    }

    fn connect_input<I: InputProps<Writer = Self::Writer> + ?Sized>(
        &mut self,
        source_input: &mut I,
        target_input: &mut I,
    ) -> Result<(), &'static str> {
        connect_input(source_input, target_input)
    }

    fn disconnect_output<I: InputProps<Writer = Self::Writer>>(
        &mut self,
        output_name: &str,
        input: &mut I,
    ) -> Result<(), &'static str> {
        let mut outputs = self.outputs_mut();
        let source_output = if let Some(out) = outputs
            .iter_mut()
            .find(|output| output.desc().name == output_name)
        {
            out
        } else {
            return Err("Output not found");
        };

        disconnect_output(*source_output, input)
    }

    fn disconnect_input<I: InputProps>(
        &mut self,
        input_name: &str,
        input: &mut I,
    ) -> Result<(), &'static str> {
        let mut inputs = self.inputs_mut();
        let source_input =
            if let Some(in_) = inputs.iter_mut().find(|input| input.name() == input_name) {
                in_
            } else {
                return Err("Input not found");
            };

        let link_id = link_id_for_input(source_input.links(), input);

        if let Some(id) = link_id {
            source_input.remove_link_by_id(&id);
            input.decrement_conn();
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
pub fn connect_output<
    Writer: Clone,
    O: Output<Writer = Writer> + ?Sized,
    I: InputProps<Writer = Writer> + ?Sized,
>(
    source_output: &mut O,
    target_input: &mut I,
) -> Result<(), &'static str> {
    // Connections to the same block and the same input are not allowed.
    if source_output.links().iter().any(|link| {
        link.target_block_id() == target_input.block_id()
            && link.target_input() == target_input.name()
    }) {
        return Err("Already connected");
    }

    let mut link = BaseLink::new(*target_input.block_id(), target_input.name().to_string());

    link.tx = Some(target_input.writer().clone());

    link.state = LinkState::Connected;

    source_output.add_link(link);
    target_input.increment_conn();

    Ok(())
}

/// Disconnect a block output from the given input
/// # Arguments
/// - source_output: The output to be disconnected
/// - target_input: The block input to be disconnected
///
/// # Returns
/// - `Ok(())` if the disconnection was successful, `Err` otherwise
pub fn disconnect_output<
    Tx: Clone,
    O: Output<Writer = Tx> + ?Sized,
    I: InputProps<Writer = Tx> + ?Sized,
>(
    source_output: &mut O,
    target_input: &mut I,
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
pub fn connect_input<I: InputProps + ?Sized>(
    source_input: &mut I,
    target_input: &mut I,
) -> Result<(), &'static str> {
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

    link.tx = Some(target_input.writer().clone());

    link.state = LinkState::Connected;

    source_input.add_link(link);
    target_input.increment_conn();

    Ok(())
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

fn link_id_for_input<I: InputProps + ?Sized>(
    links: Vec<&dyn Link>,
    target_input: &I,
) -> Option<Uuid> {
    let link_id = links
        .iter()
        .find(|link| {
            link.target_input() == target_input.name()
                && link.target_block_id() == target_input.block_id()
        })
        .map(|link| *link.id());
    link_id
}
