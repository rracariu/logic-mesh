//! Verifies that native blocks can be defined in a downstream crate
//! using the same `#[block]` / `#[derive(BlockProps)]` machinery as the
//! built-in blocks.

use logic_mesh::base::block::Block;
use logic_mesh::base::input::{InputProps, InputReader};
use logic_mesh::base::output::Output;
use logic_mesh::blocks::{InputImpl, OutputImpl};
use logic_mesh::{BlockProps, block};

/// Doubles the value of its numeric input.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Double"]
#[library = "downstream"]
#[category = "math"]
pub struct Double {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Double {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(value) = self.input.get_value() {
            let num: f64 = match value.try_into() {
                Ok(num) => num,
                Err(_) => return,
            };
            self.out.set((num * 2.0).into());
        }
    }
}

#[allow(missing_docs)]
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Bare"]
#[library = "downstream"]
#[category = "test"]
pub struct Bare {
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Bare {
    async fn execute(&mut self) {}
}

/// Same as [`Double`] but declares its pins with fully qualified paths.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Triple"]
#[library = "downstream"]
#[category = "math"]
pub struct Triple {
    #[input(name = "in", kind = "Number")]
    pub input: logic_mesh::blocks::InputImpl,
    #[output(kind = "Number")]
    pub out: logic_mesh::blocks::OutputImpl,
}

impl Block for Triple {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(value) = self.input.get_value() {
            let num: f64 = match value.try_into() {
                Ok(num) => num,
                Err(_) => return,
            };
            self.out.set((num * 3.0).into());
        }
    }
}

/// Custom field types whose names merely start with `InputImpl` /
/// `OutputImpl` must not be classified as pins by the derive.
#[derive(Debug, Default)]
pub struct InputImplConfig {
    /// The scale factor to apply to the input value.
    pub scale: f64,
}

/// Custom field types whose names merely start with `InputImpl` /
/// `OutputImpl` must not be classified as pins by the derive.
#[derive(Debug, Default)]
pub struct OutputImplSettings {
    /// The offset to apply to the output value.
    pub offset: f64,
}

/// Has one real pin of each kind plus custom fields with pin-like
/// type-name prefixes.
#[block]
#[derive(BlockProps, Debug)]
#[dis = "Prefixed"]
#[library = "downstream"]
#[category = "math"]
pub struct Prefixed {
    #[input(name = "in", kind = "Number")]
    pub input: InputImpl,
    #[output(kind = "Number")]
    pub out: OutputImpl,
    pub config: InputImplConfig,
    pub settings: OutputImplSettings,
}

impl Block for Prefixed {
    async fn execute(&mut self) {
        self.read_inputs_until_ready().await;

        if let Some(value) = self.input.get_value() {
            let num: f64 = match value.try_into() {
                Ok(num) => num,
                Err(_) => return,
            };
            self.out
                .set((num * self.config.scale + self.settings.offset).into());
        }
    }
}

/// Declares every attribute the derive reads.
///
/// Next to them sits an attribute of the same `#[name = "..."]` shape
/// that the derive must leave alone.
#[block]
#[derive(BlockProps, Debug)]
#[must_use = "a block does nothing until it is scheduled"]
#[dis = "Every attribute"]
#[library = "downstream"]
#[category = "test"]
#[ver = "2.1.0"]
#[input(name = "x", kind = "Number", count = 2)]
pub struct Attributed {
    #[output(name = "y", kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Attributed {
    async fn execute(&mut self) {}
}

/// Declares its pins with bare `#[input]` / `#[output]` attributes.
#[block]
#[derive(BlockProps, Debug)]
#[library = "downstream"]
#[category = "test"]
pub struct BarePins {
    #[input]
    pub a: InputImpl,
    #[output]
    pub out: OutputImpl,
}

impl Block for BarePins {
    async fn execute(&mut self) {}
}

/// Pads its attribute values with whitespace.
#[block]
#[derive(BlockProps, Debug)]
#[dis = " Padded "]
#[library = "downstream "]
#[category = " test"]
#[ver = " 1.2.3 "]
pub struct Padded {
    #[output(kind = "Number")]
    pub out: OutputImpl,
}

impl Block for Padded {
    async fn execute(&mut self) {}
}

/// Names its pins after fields whose names are keywords, and one kind
/// with a raw identifier.
#[block]
#[derive(BlockProps, Debug)]
#[library = "downstream"]
#[category = "test"]
pub struct Keywords {
    #[input(kind = "Number")]
    pub r#in: InputImpl,
    #[output(kind = "r#Number")]
    pub r#type: OutputImpl,
}

impl Block for Keywords {
    async fn execute(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use logic_mesh::base::block::BlockProps;
    use logic_mesh::base::output::OutputProps;

    #[test]
    fn desc_reflects_attributes() {
        use logic_mesh::base::block::BlockStaticDesc;

        let desc = <Double as BlockStaticDesc>::static_desc();
        assert_eq!(desc.name, "Double");
        assert_eq!(desc.dis, "Double");
        assert_eq!(desc.library, "downstream");
        assert_eq!(desc.category, "math");
        assert_eq!(desc.inputs.len(), 1);
        assert_eq!(desc.inputs[0].name, "in");
        assert_eq!(desc.outputs.len(), 1);
        assert_eq!(desc.outputs[0].name, "out");
    }

    #[tokio::test]
    async fn executes_and_doubles_input() {
        let mut block = Double::new();

        assert_eq!(block.inputs().len(), 1);
        assert_eq!(block.outputs().len(), 1);

        let input = block.get_input_mut("in").unwrap();
        input.increment_conn();
        input
            .writer()
            .send((21.into(), logic_mesh::base::Status::Ok))
            .unwrap();

        block.execute().await;
        let expected: logic_mesh::Value = 42.into();
        assert_eq!(block.out.value(), &expected);
    }

    #[tokio::test]
    async fn fully_qualified_pin_types_work() {
        use logic_mesh::base::block::BlockStaticDesc;

        let desc = <Triple as BlockStaticDesc>::static_desc();
        assert_eq!(desc.inputs.len(), 1);
        assert_eq!(desc.outputs.len(), 1);

        let mut block = Triple::new();
        let input = block.get_input_mut("in").unwrap();
        input.increment_conn();
        input
            .writer()
            .send((7.into(), logic_mesh::base::Status::Ok))
            .unwrap();

        block.execute().await;
        assert_eq!(block.out.value(), &21.into());
    }

    #[tokio::test]
    async fn prefixed_field_types_are_not_pins() {
        use logic_mesh::base::block::BlockStaticDesc;

        let desc = <Prefixed as BlockStaticDesc>::static_desc();
        assert_eq!(desc.inputs.len(), 1);
        assert_eq!(desc.outputs.len(), 1);

        let mut block = Prefixed::new();
        block.config = InputImplConfig { scale: 2.0 };
        block.settings = OutputImplSettings { offset: 1.0 };

        assert_eq!(block.inputs().len(), 1);
        assert_eq!(block.outputs().len(), 1);

        let input = block.get_input_mut("in").unwrap();
        input.increment_conn();
        input
            .writer()
            .send((10.into(), logic_mesh::base::Status::Ok))
            .unwrap();

        block.execute().await;
        assert_eq!(block.out.value(), &21.into());
    }

    #[test]
    fn doc_comment_captured_in_desc() {
        use logic_mesh::base::block::BlockStaticDesc;

        let desc = <Double as BlockStaticDesc>::static_desc();
        assert_eq!(
            desc.doc, "Doubles the value of its numeric input.",
            "/// doc comment should be captured in desc().doc"
        );
    }

    /// Regression: the lines of a doc comment were joined with no
    /// separator, gluing the last word of one line to the first of the
    /// next.
    #[test]
    fn doc_comment_lines_joined_by_newlines() {
        use logic_mesh::base::block::BlockStaticDesc;

        let desc = <Prefixed as BlockStaticDesc>::static_desc();
        assert_eq!(
            desc.doc,
            "Has one real pin of each kind plus custom fields with pin-like\ntype-name prefixes."
        );

        let desc = <Attributed as BlockStaticDesc>::static_desc();
        assert_eq!(
            desc.doc,
            "Declares every attribute the derive reads.\n\n\
             Next to them sits an attribute of the same `#[name = \"...\"]` shape\n\
             that the derive must leave alone."
        );
    }

    /// Regression: `#[ver]` was read but not registered, so it could not
    /// be set; and any other `#[name = "..."]` attribute, such as
    /// `#[must_use = "..."]`, was taken for a description field and
    /// failed to compile.
    #[test]
    fn desc_reflects_every_attribute() {
        use logic_mesh::base::block::BlockStaticDesc;

        let desc = <Attributed as BlockStaticDesc>::static_desc();
        assert_eq!(desc.name, "Attributed");
        assert_eq!(desc.dis, "Every attribute");
        assert_eq!(desc.library, "downstream");
        assert_eq!(desc.category, "test");
        assert_eq!(desc.ver, "2.1.0");
        let inputs: Vec<_> = desc.inputs.iter().map(|pin| pin.name.as_str()).collect();
        assert_eq!(inputs, ["x0", "x1"]);
        assert_eq!(desc.outputs[0].name, "y");

        let block = Attributed::new();
        assert_eq!(block.inputs().len(), 2);
        assert_eq!(block.outputs()[0].name(), "y");
    }

    /// A bare `#[input]` / `#[output]` declares a pin named after its
    /// field, of kind `Null`. The derive used to ignore it and leave the
    /// field a default-built `InputImpl` / `OutputImpl` that was no pin.
    #[test]
    fn bare_pin_attributes_declare_pins() {
        use logic_mesh::HaystackKind;
        use logic_mesh::base::block::{BlockPin, BlockStaticDesc};

        let desc = <BarePins as BlockStaticDesc>::static_desc();
        assert_eq!(desc.inputs, [BlockPin::new("a", HaystackKind::Null)]);
        assert_eq!(desc.outputs, [BlockPin::new("out", HaystackKind::Null)]);

        let block = BarePins::new();
        let inputs: Vec<_> = block.inputs().iter().map(|pin| pin.name()).collect();
        assert_eq!(inputs, ["a"]);
        let outputs: Vec<_> = block.outputs().iter().map(|pin| pin.name()).collect();
        assert_eq!(outputs, ["out"]);
    }

    /// Regression: the derive's rewrite took attribute values verbatim,
    /// so the whitespace around `#[library = "downstream "]` became part
    /// of the block's registry key, where it had always been trimmed.
    #[test]
    fn attribute_values_are_trimmed() {
        use logic_mesh::base::block::BlockStaticDesc;

        let desc = <Padded as BlockStaticDesc>::static_desc();
        assert_eq!(desc.dis, "Padded");
        assert_eq!(desc.library, "downstream");
        assert_eq!(desc.category, "test");
        assert_eq!(desc.ver, "1.2.3");
        assert_eq!(desc.qname(), "downstream::Padded");
    }

    /// Regression: a pin named after a raw field identifier kept its
    /// `r#` prefix, so the field `r#in` declared a pin named `r#in`.
    #[test]
    fn raw_field_names_name_pins_without_the_prefix() {
        use logic_mesh::base::block::BlockStaticDesc;

        let desc = <Keywords as BlockStaticDesc>::static_desc();
        assert_eq!(desc.inputs[0].name, "in");
        assert_eq!(desc.outputs[0].name, "type");

        let block = Keywords::new();
        assert_eq!(block.inputs()[0].name(), "in");
        assert_eq!(block.outputs()[0].name(), "type");
    }

    /// Regression: `kind = "r#Number"` made the derive panic, where it
    /// had always named the `Number` variant.
    #[test]
    fn raw_kind_names_the_variant() {
        use logic_mesh::HaystackKind;
        use logic_mesh::base::block::BlockStaticDesc;

        let desc = <Keywords as BlockStaticDesc>::static_desc();
        assert_eq!(desc.outputs[0].kind, HaystackKind::Number);
    }

    /// A block written by hand describes itself with the builder:
    /// `BlockDesc` is `#[non_exhaustive]`, so a downstream crate cannot
    /// name its fields in a struct literal.
    #[test]
    fn hand_written_desc_uses_the_builder() {
        use logic_mesh::HaystackKind;
        use logic_mesh::base::block::{BlockDesc, BlockPin, BlockStaticDesc};

        let desc = BlockDesc::new("Double", "downstream")
            .with_category("math")
            .with_doc("Doubles the value of its numeric input.")
            .with_input("in", HaystackKind::Number)
            .with_output("out", HaystackKind::Number);

        assert_eq!(&desc, <Double as BlockStaticDesc>::static_desc());
        assert_eq!(desc.inputs, [BlockPin::new("in", HaystackKind::Number)]);
    }

    #[test]
    fn missing_doc_defaults_to_empty() {
        use logic_mesh::base::block::BlockStaticDesc;

        let desc = <Bare as BlockStaticDesc>::static_desc();
        assert_eq!(
            desc.doc, "",
            "block without /// doc should have an empty desc().doc"
        );
    }

    #[test]
    fn registers_in_runtime_registry() {
        let _ = logic_mesh::blocks::registry::register::<Double>();

        let registered = logic_mesh::blocks::registry::list_registered_blocks();
        assert!(
            registered
                .iter()
                .any(|desc| desc.name == "Double" && desc.library == "downstream")
        );
    }

    #[tokio::test]
    async fn registered_block_evals_by_name() {
        let _ = logic_mesh::blocks::registry::register::<Double>();

        let result = logic_mesh::blocks::registry::eval_static_block(
            "Double",
            Some("downstream"),
            vec![21.into()],
        )
        .await;
        assert_eq!(result.unwrap(), vec![logic_mesh::Value::from(42)]);
    }

    #[test]
    fn registered_block_schedules_on_engine() {
        let _ = logic_mesh::blocks::registry::register::<Double>();

        let mut eng = logic_mesh::single_threaded::SingleThreadedEngine::new();
        let id =
            logic_mesh::blocks::registry::schedule_block("Double", Some("downstream"), &mut eng)
                .expect("schedule registered block");

        assert!(
            eng.block_handles()
                .iter()
                .any(|b| *b.id() == id && b.desc().name == "Double")
        );
    }

    /// The engine-generic entry point also serves the MT engine: its
    /// `Send` requirement is part of the `Engine::schedule` bound.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn registered_block_schedules_on_multi_threaded_engine() {
        let _ = logic_mesh::blocks::registry::register::<Double>();

        let mut eng = logic_mesh::multi_threaded::MultiThreadedEngine::new();
        let id =
            logic_mesh::blocks::registry::schedule_block("Double", Some("downstream"), &mut eng)
                .expect("schedule registered block on MT engine");

        assert!(eng.block_handles().iter().any(|b| *b.id() == id));
    }
}
