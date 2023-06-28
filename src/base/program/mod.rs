// Copyright (c) 2022-2023, Radu Racariu.

//! Module for defining the program
//! that would be executed by the engine.

pub mod data;

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

use crate::blocks::{registry::BLOCKS, InputImpl};

use self::data::{BlockData, LinkData, ProgramMeta};

use super::{engine::Engine, input::InputProps};

type Reader = <InputImpl as InputProps>::Reader;
type Writer = <InputImpl as InputProps>::Writer;

pub trait EngineType = Engine<Reader = Reader, Writer = Writer> + Default;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Program<E: EngineType> {
    pub meta: ProgramMeta,
    pub blocks: Vec<BlockData>,
    pub links: Vec<LinkData>,
    pub engine: E,
}

impl<E: EngineType> Program<E> {
    pub fn new(name: &str, blocks: Vec<BlockData>, links: Vec<LinkData>) -> Self {
        Self {
            meta: ProgramMeta {
                name: name.to_string(),
                ..Default::default()
            },
            blocks,
            links,
            engine: E::default(),
        }
    }

    pub fn load(&mut self) -> Result<()> {
        if let Ok(reg) = BLOCKS.lock() {
            if !self.blocks.iter().all(|b| reg.contains_key(&b.name)) {
                return Err(anyhow!("Block not found"));
            }
        } else {
            return Err(anyhow!("Block registry is locked"));
        }

        self.engine.load_blocks_and_links(&self.blocks, &self.links)
    }

    pub async fn run(&mut self) {
        self.engine.run().await
    }
}

#[cfg(test)]
mod test {
    use crate::single_threaded::SingleThreadedEngine;

    use super::{
        data::{BlockData, LinkData},
        Program,
    };

    #[test]
    fn test_program_load() {
        let mut program = Program::<SingleThreadedEngine>::new(
            "test",
            vec![
                BlockData {
                    id: "00000000-0000-0000-0000-000000000000".to_string(),
                    name: "Add".to_string(),
                    dis: "Add".to_string(),
                    lib: "test".to_string(),
                    category: "maths".to_string(),
                    ver: "0.1.0".to_string(),
                },
                BlockData {
                    id: "00000000-0000-0000-0000-000000000001".to_string(),
                    name: "Add".to_string(),
                    dis: "Add".to_string(),
                    lib: "test".to_string(),
                    category: "maths".to_string(),
                    ver: "0.1.0".to_string(),
                },
            ],
            vec![LinkData {
                id: None,
                source_block_uuid: "00000000-0000-0000-0000-000000000000".to_string(),
                target_block_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                source_block_pin_name: "out".to_string(),
                target_block_pin_name: "in1".to_string(),
            }],
        );

        assert!(program.load().is_ok());

        assert!(program.engine.blocks().iter().all(|b| b.name() == "Add"));

        assert!(program.engine.blocks()[0]
            .get_output("out")
            .unwrap()
            .is_connected());

        assert!(
            program.engine.blocks()[0]
                .get_output("out")
                .unwrap()
                .links()
                .len()
                == 1
        );

        assert!(program.engine.blocks()[0]
            .get_output("out")
            .unwrap()
            .links()
            .iter()
            .any(
                |l| l.target_block_id().to_string() == "00000000-0000-0000-0000-000000000001"
                    && l.target_input() == "in1"
            ));
    }

    #[test]
    fn test_program_load_invalid_block() {
        let mut program = Program::<SingleThreadedEngine>::new(
            "test",
            vec![BlockData {
                id: "00000000-0000-0000-0000-000000000000".to_string(),
                name: "Missing".to_string(),
                dis: "Missing".to_string(),
                lib: "test".to_string(),
                category: "maths".to_string(),
                ver: "0.1.0".to_string(),
            }],
            vec![],
        );

        assert!(program.load().is_err());
    }

    #[test]
    fn test_program_load_invalid_link() {
        let blocks = vec![
            BlockData {
                id: "00000000-0000-0000-0000-000000000000".to_string(),
                name: "Add".to_string(),
                dis: "Add".to_string(),
                lib: "test".to_string(),
                category: "maths".to_string(),
                ver: "0.1.0".to_string(),
            },
            BlockData {
                id: "00000000-0000-0000-0000-000000000001".to_string(),
                name: "Add".to_string(),
                dis: "Add".to_string(),
                lib: "test".to_string(),
                category: "maths".to_string(),
                ver: "0.1.0".to_string(),
            },
        ];

        // Invalid source block
        let mut program = Program::<SingleThreadedEngine>::new(
            "test",
            blocks.clone(),
            vec![LinkData {
                id: None,
                source_block_uuid: "00000000-0000-0000-0000-000000000009".to_string(),
                target_block_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                source_block_pin_name: "out".to_string(),
                target_block_pin_name: "in1".to_string(),
            }],
        );

        assert!(program.load().is_err());

        // Invalid source block pin
        let mut program = Program::<SingleThreadedEngine>::new(
            "test",
            blocks.clone(),
            vec![LinkData {
                id: None,
                source_block_uuid: "00000000-0000-0000-0000-000000000000".to_string(),
                target_block_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                source_block_pin_name: "missing".to_string(),
                target_block_pin_name: "in1".to_string(),
            }],
        );

        assert!(program.load().is_err());

        // Invalid target input
        let mut program = Program::<SingleThreadedEngine>::new(
            "test",
            blocks.clone(),
            vec![LinkData {
                id: None,
                source_block_uuid: "00000000-0000-0000-0000-000000000000".to_string(),
                target_block_uuid: "00000000-0000-0000-0000-000000000001".to_string(),
                source_block_pin_name: "out".to_string(),
                target_block_pin_name: "missing".to_string(),
            }],
        );

        assert!(program.load().is_err());
    }
}
