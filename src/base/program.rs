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

pub mod data;

pub use data::{BlockData, LinkData, PinValue, Position, Program, ProgramBlock, ProgramMeta};

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
            lib: lib.to_string(),
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
                    lib: "core".to_string(),
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
                lib: "core".to_string(),
                label: Some("Demand sum".to_string()),
                positions: Some(Position { x: 120.5, y: 80.0 }),
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
