// Copyright (c) 2022-2026, Radu Racariu.

//! Builder for assembling a [`Program`] in code.

use std::collections::BTreeMap;

use libhaystack::val::Value;
use uuid::Uuid;

use crate::base::block::BlockDesc;
use crate::base::error::{EngineError, LinkEnd, RegistryError, Result};
use crate::blocks::registry::{CORE_LIB, get_block};

use super::data::{LinkData, PinValue, Position, Program, ProgramBlock};

/// A block added to a [`ProgramBuilder`], used to refer to it when
/// linking.
///
/// Only handed out by the builder, so a link names a block that was
/// added to a program rather than an arbitrary id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockRef(Uuid);

impl BlockRef {
    /// The block's id — its key in [`Program::blocks`].
    pub fn id(&self) -> Uuid {
        self.0
    }
}

/// Assembles a [`Program`] in code, checking each block, pin and link
/// against the block registry as it is added — so the result loads
/// without the lookup failures a hand-assembled program can hit.
///
/// # Examples
///
/// ```
/// use logic_mesh::base::program::ProgramBuilder;
///
/// let mut builder = ProgramBuilder::new("sine + sum");
/// let sine = builder
///     .add_block("SineWave")?
///     .label("fast sine")
///     .input("freq", 50)?
///     .finish();
/// let sum = builder.add_block("Add")?.position(200.0, 50.0).finish();
/// builder.link(sine, "out", sum, "in0")?;
///
/// let program = builder.build();
/// assert_eq!(program.blocks.len(), 2);
/// assert_eq!(program.links.len(), 1);
/// # Ok::<(), logic_mesh::Error>(())
/// ```
#[derive(Debug, Default)]
pub struct ProgramBuilder {
    program: Program,
    /// Descriptor of every added block, to validate pins against.
    descs: BTreeMap<Uuid, BlockDesc>,
}

impl ProgramBuilder {
    /// Starts an empty program named `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            program: Program {
                name: Some(name.into()),
                ..Default::default()
            },
            descs: BTreeMap::new(),
        }
    }

    /// Sets the program's human-readable description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.program.description = Some(description.into());
        self
    }

    /// Adds a block from the core library.
    ///
    /// # Errors
    ///
    /// Returns an error if no such block is registered.
    pub fn add_block(&mut self, name: &str) -> Result<ProgramBlockBuilder<'_>> {
        self.add_block_in(CORE_LIB, name)
    }

    /// Adds a block from library `lib`.
    ///
    /// # Errors
    ///
    /// Returns an error if no such block is registered.
    pub fn add_block_in(&mut self, lib: &str, name: &str) -> Result<ProgramBlockBuilder<'_>> {
        let entry = get_block(name, Some(lib)).ok_or_else(|| RegistryError::BlockNotFound {
            library: lib.to_string(),
            name: name.to_string(),
        })?;

        let id = Uuid::new_v4();
        self.program.blocks.insert(
            id.to_string(),
            ProgramBlock {
                name: name.to_string(),
                lib: lib.to_string(),
                ..Default::default()
            },
        );
        self.descs.insert(id, entry.desc);

        Ok(ProgramBlockBuilder { builder: self, id })
    }

    /// Links `source_pin` of `source` to input `target_pin` of `target`,
    /// returning the link's id. The source pin can be an output or, to
    /// fan an input out, an input.
    ///
    /// # Errors
    ///
    /// Returns an error if either block was not added to this builder or
    /// names no such pin.
    pub fn link(
        &mut self,
        source: BlockRef,
        source_pin: &str,
        target: BlockRef,
        target_pin: &str,
    ) -> Result<Uuid> {
        let source_desc = self.desc(source)?;
        let source_exists = source_desc.outputs.iter().any(|o| o.name == source_pin)
            || source_desc.inputs.iter().any(|i| i.name == source_pin);
        if !source_exists {
            return Err(EngineError::PinNotFound {
                end: LinkEnd::Source,
                block: source.id(),
                pin: source_pin.to_string(),
            }
            .into());
        }

        let target_desc = self.desc(target)?;
        if !target_desc.inputs.iter().any(|i| i.name == target_pin) {
            return Err(EngineError::PinNotFound {
                end: LinkEnd::Target,
                block: target.id(),
                pin: target_pin.to_string(),
            }
            .into());
        }

        let id = Uuid::new_v4();
        self.program.links.insert(
            id.to_string(),
            LinkData {
                id: Some(id.to_string()),
                source_block_uuid: source.id().to_string(),
                target_block_uuid: target.id().to_string(),
                source_block_pin_name: source_pin.to_string(),
                target_block_pin_name: target_pin.to_string(),
            },
        );
        Ok(id)
    }

    /// Returns the assembled program.
    pub fn build(self) -> Program {
        self.program
    }

    fn desc(&self, block: BlockRef) -> Result<&BlockDesc, EngineError> {
        self.descs
            .get(&block.id())
            .ok_or(EngineError::BlockInstanceNotFound { id: block.id() })
    }
}

/// Configures a block just added to a [`ProgramBuilder`]; [`finish`](Self::finish)
/// returns the [`BlockRef`] to link it with.
#[derive(Debug)]
pub struct ProgramBlockBuilder<'a> {
    builder: &'a mut ProgramBuilder,
    id: Uuid,
}

impl ProgramBlockBuilder<'_> {
    /// Sets the display label shown alongside the block-type name.
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.block().label = Some(label.into());
        self
    }

    /// Sets the block's UI position.
    pub fn position(mut self, x: f64, y: f64) -> Self {
        self.block().positions = Some(Position { x, y });
        self
    }

    /// Sets a constant value on input `name`, written to the block when
    /// the program is loaded.
    ///
    /// # Errors
    ///
    /// Returns an error if the block has no such input.
    pub fn input(mut self, name: &str, value: impl Into<Value>) -> Result<Self> {
        let desc = &self.builder.descs[&self.id];
        if !desc.inputs.iter().any(|i| i.name == name) {
            return Err(EngineError::InputNotFound {
                block: self.id,
                pin: name.to_string(),
            }
            .into());
        }

        self.block().inputs.insert(
            name.to_string(),
            PinValue {
                value: value.into(),
                is_connected: false,
            },
        );
        Ok(self)
    }

    /// Returns the handle to refer to this block by.
    pub fn finish(self) -> BlockRef {
        BlockRef(self.id)
    }

    fn block(&mut self) -> &mut ProgramBlock {
        self.builder
            .program
            .blocks
            .get_mut(&self.id.to_string())
            .expect("added by `add_block_in`")
    }
}

#[cfg(test)]
mod test {
    use assert_matches::assert_matches;

    use super::*;
    use crate::base::engine::Engine;
    use crate::base::error::Error;
    use crate::single_threaded::SingleThreadedEngine;

    #[test]
    fn built_program_schedules_with_its_metadata() {
        let mut builder = ProgramBuilder::new("demo").description("two adders");
        let a = builder
            .add_block("Add")
            .unwrap()
            .label("first")
            .position(10.0, 20.0)
            .input("in1", 5)
            .unwrap()
            .finish();
        let b = builder.add_block("Add").unwrap().finish();
        let link = builder.link(a, "out", b, "in0").unwrap();

        let program = builder.build();
        assert_eq!(program.name.as_deref(), Some("demo"));
        assert_eq!(program.description.as_deref(), Some("two adders"));
        let first = &program.blocks[&a.id().to_string()];
        assert_eq!(first.inputs["in1"].value, 5.into());
        assert_eq!(
            program.links[&link.to_string()].target_block_uuid,
            b.id().to_string()
        );

        let mut engine = SingleThreadedEngine::new();
        engine
            .schedule_program_blocks(&program)
            .expect("a built program schedules");
        let handle = engine.block_handle(&a.id()).expect("scheduled");
        assert_eq!(handle.label(), Some("first"));
        assert_eq!(handle.position(), Some(Position { x: 10.0, y: 20.0 }));
    }

    #[test]
    fn unknown_names_are_rejected_where_they_are_added() {
        let mut builder = ProgramBuilder::new("errors");

        assert_matches!(
            builder.add_block("NoSuchBlock").map(|b| b.finish()),
            Err(Error::Registry(RegistryError::BlockNotFound { .. }))
        );

        let add = builder.add_block("Add").unwrap().finish();
        let err = builder
            .add_block("Add")
            .unwrap()
            .input("no_such_input", 1)
            .expect_err("unknown input");
        assert_matches!(
            err,
            Error::Engine(EngineError::InputNotFound { pin, .. }) if pin == "no_such_input"
        );

        assert_matches!(
            builder.link(add, "nope", add, "in0"),
            Err(Error::Engine(EngineError::PinNotFound {
                end: LinkEnd::Source,
                ..
            }))
        );
        // Outputs are not link targets.
        assert_matches!(
            builder.link(add, "out", add, "out"),
            Err(Error::Engine(EngineError::PinNotFound {
                end: LinkEnd::Target,
                ..
            }))
        );
    }

    #[test]
    fn blocks_of_another_builder_cannot_be_linked() {
        let mut other = ProgramBuilder::new("other");
        let foreign = other.add_block("Add").unwrap().finish();

        let mut builder = ProgramBuilder::new("this");
        let add = builder.add_block("Add").unwrap().finish();
        assert_matches!(
            builder.link(foreign, "out", add, "in0"),
            Err(Error::Engine(EngineError::BlockInstanceNotFound { id })) if id == foreign.id()
        );
    }
}
