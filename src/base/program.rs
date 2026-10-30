// Copyright (c) 2022-2026, Radu Racariu.

//! Program save/load format.
//!
//! [`Program`] is the canonical, fully-self-describing shape an engine
//! can serialize out and load back in — including pin values (constants)
//! and UI metadata (label, position). Previous versions exposed a
//! `Program<E>` runner type that combined a save format with an engine
//! instance; that fused two concerns and made the saveable format a
//! generic. The new [`Program`] is plain data — engine instances are
//! managed separately and load it via [`crate::base::engine::Engine`].
//! To assemble one in code, use [`ProgramBuilder`].
//!
//! # Format versions
//!
//! A program records the format it was written in as
//! [`version`](Program::version):
//!
//! - **1** — the format this crate writes
//!   ([`Program::FORMAT_VERSION`]). Each block saves its type's version
//!   ([`ProgramBlock::ver`]) and may carry opaque editor metadata
//!   ([`ProgramBlock::editor`]); its UI position is keyed `position`.
//! - **missing (read as 0)** — a save from before format versions,
//!   written by logic-mesh 1.x. It is the same shape as version 1
//!   without `ver` and `editor`, with the position keyed `positions`,
//!   which [`ProgramBlock::position`] still reads, so it loads
//!   unchanged. A save of the loaded program writes version 1.
//!
//! A program from a newer format fails to deserialize, with the message
//! of
//! [`EngineError::UnsupportedProgramVersion`](crate::EngineError::UnsupportedProgramVersion),
//! before its blocks and links are read, so a format that changed their
//! shape is still reported by its version. An engine rejects one built
//! in code with that error before it schedules any of its blocks.

pub(crate) mod builder;
pub(crate) mod data;

pub use builder::{BlockRef, ProgramBlockBuilder, ProgramBuilder};
pub use data::{EditorMetadata, LinkData, PinValue, Position, Program, ProgramBlock};

#[cfg(test)]
mod test {
    use crate::base::engine::Engine;
    use crate::single_threaded::SingleThreadedEngine;

    use super::{
        Program, ProgramBlock,
        data::{LinkData, Position},
    };

    fn add_block(lib: &str) -> ProgramBlock {
        ProgramBlock {
            name: "Add".to_string(),
            library: lib.to_string(),
            ..Default::default()
        }
    }

    fn program_with(blocks: Vec<(&str, ProgramBlock)>, links: Vec<(&str, LinkData)>) -> Program {
        let mut p = Program::default();
        for (id, b) in blocks {
            p.blocks.insert(id.to_string(), b);
        }
        for (id, l) in links {
            p.links.insert(id.to_string(), l);
        }
        p
    }

    #[test]
    fn test_program_load_schedules_blocks() {
        let program = program_with(
            vec![
                ("00000000-0000-0000-0000-000000000000", add_block("core")),
                ("00000000-0000-0000-0000-000000000001", add_block("core")),
            ],
            vec![(
                "00000000-0000-0000-0000-0000000000aa",
                LinkData {
                    id: Some("00000000-0000-0000-0000-0000000000aa".to_string()),
                    source_block_uuid: "00000000-0000-0000-0000-000000000000".to_string(),
                    target_block_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                    source_block_pin_name: "out".to_string(),
                    target_block_pin_name: "in1".to_string(),
                },
            )],
        );

        let mut engine = SingleThreadedEngine::new();
        // Sync portion: schedules every block and queues the links for
        // wiring during run(). Value writes are async and require the
        // actor loop to be live; covered in the integration tests.
        engine
            .schedule_program_blocks(&program)
            .expect("schedule blocks");

        assert!(engine.block_handles().iter().all(|b| b.name() == "Add"));
    }

    #[test]
    fn test_program_load_invalid_block_name() {
        let program = program_with(
            vec![(
                "00000000-0000-0000-0000-000000000000",
                ProgramBlock {
                    name: "Missing".to_string(),
                    library: "core".to_string(),
                    ..Default::default()
                },
            )],
            vec![],
        );

        let mut engine = SingleThreadedEngine::new();
        assert!(engine.schedule_program_blocks(&program).is_err());
    }

    #[test]
    fn test_program_load_invalid_link_pins() {
        let blocks = vec![
            ("00000000-0000-0000-0000-000000000000", add_block("core")),
            ("00000000-0000-0000-0000-000000000001", add_block("core")),
        ];

        // Invalid source pin name.
        {
            let program = program_with(
                blocks.clone(),
                vec![(
                    "00000000-0000-0000-0000-0000000000aa",
                    LinkData {
                        id: None,
                        source_block_uuid: "00000000-0000-0000-0000-000000000000".to_string(),
                        target_block_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                        source_block_pin_name: "missing".to_string(),
                        target_block_pin_name: "in1".to_string(),
                    },
                )],
            );
            let mut engine = SingleThreadedEngine::new();
            assert!(engine.schedule_program_blocks(&program).is_err());
        }

        // Invalid target input.
        {
            let program = program_with(
                blocks.clone(),
                vec![(
                    "00000000-0000-0000-0000-0000000000bb",
                    LinkData {
                        id: None,
                        source_block_uuid: "00000000-0000-0000-0000-000000000000".to_string(),
                        target_block_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                        source_block_pin_name: "out".to_string(),
                        target_block_pin_name: "missing".to_string(),
                    },
                )],
            );
            let mut engine = SingleThreadedEngine::new();
            assert!(engine.schedule_program_blocks(&program).is_err());
        }
    }

    #[test]
    fn test_program_block_stores_label_and_position() {
        // The engine should round-trip the per-block UI metadata
        // (label + position) without any external loader involvement.
        let mut program = Program::default();
        program.blocks.insert(
            "00000000-0000-0000-0000-000000000000".to_string(),
            ProgramBlock {
                name: "Add".to_string(),
                library: "core".to_string(),
                label: Some("Demand sum".to_string()),
                position: Some(Position { x: 120.5, y: 80.0 }),
                ..Default::default()
            },
        );

        let mut engine = SingleThreadedEngine::new();
        engine
            .schedule_program_blocks(&program)
            .expect("schedule blocks");

        let handle = engine
            .block_handle(&uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000000").unwrap())
            .expect("handle");
        assert_eq!(handle.label(), Some("Demand sum"));
        let pos = handle.position().expect("position stored");
        assert!((pos.x - 120.5).abs() < f64::EPSILON);
    }
}
