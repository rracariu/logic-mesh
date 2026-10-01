// Copyright (c) 2022-2024, Radu Racariu.

//! Block registry.

use crate::base::block::{
    Block, BlockConstruct, BlockDesc, BlockInput, BlockOutput, BlockProps, BlockState,
    BlockStaticDesc,
};
use crate::base::input::input_reader::InputReader;
use libhaystack::val::Value;

use crate::base::engine::{Engine, MaybeSendSync};

use crate::base::error::{RegistryError, Result};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::LazyLock;
use std::sync::Mutex;
use uuid::Uuid;

use crate::blocks::{ReaderImpl, WriterImpl};

/// The library name of the built-in blocks.
pub const CORE_LIB: &str = "core";

pub(crate) type DynBlockProps = dyn BlockProps<Reader = ReaderImpl, Writer = WriterImpl>;
type MapType = HashMap<String, HashMap<String, BlockEntry>>;
type BlockRegistry = Mutex<MapType>;

/// A block registration entry in the registry.
#[derive(Debug, Clone)]
pub struct BlockEntry {
    /// Block descriptor (name, library, pins, etc.).
    pub desc: BlockDesc,
    /// How to instantiate the block. [`None`] for a description-only
    /// entry (see [`register_block_desc`]), whose implementation lives
    /// outside the registry — e.g. a JS block.
    factory: Option<BlockFactory>,
}

impl BlockEntry {
    /// Creates a new instance of this block, or [`None`] for a
    /// description-only entry.
    pub fn make(&self) -> Option<Box<DynBlockProps>> {
        self.factory.map(|factory| (factory.make)())
    }
}

/// The constructors of a type-registered block, held as one unit so an
/// entry can either build its block both ways or not at all.
#[derive(Debug, Clone, Copy)]
struct BlockFactory {
    /// Builds the block's properties as a trait object.
    make: fn() -> Box<DynBlockProps>,
    /// Builds a schedulable, type-erased instance, with a fixed id if
    /// one is given.
    make_erased: fn(Option<Uuid>) -> RegisteredBlock,
}

/// Receives a block the registry constructed by name, as its concrete
/// type — so statically compiled blocks keep static dispatch all the
/// way into their actor task.
///
/// Implemented by the engines in every phase (blocks are added by name
/// both while configuring and while running) and, for the public
/// [`schedule_block`] entry points, by [`Engine`] implementors.
pub(crate) trait BlockSink {
    /// Takes ownership of `block`, typically scheduling it.
    fn accept<B>(&mut self, block: B) -> Result<()>
    where
        B: Block<Reader = ReaderImpl, Writer = WriterImpl> + MaybeSendSync + 'static;
}

/// [`BlockSink`] that schedules through the public [`Engine`] trait.
struct ScheduleOn<'a, E>(&'a mut E);

impl<E> BlockSink for ScheduleOn<'_, E>
where
    E: Engine<Reader = ReaderImpl, Writer = WriterImpl>,
{
    fn accept<B>(&mut self, block: B) -> Result<()>
    where
        B: Block<Reader = ReaderImpl, Writer = WriterImpl> + MaybeSendSync + 'static,
    {
        self.0.schedule(block)
    }
}

/// Object-safe view over [`Block`] so runtime-registered blocks can be
/// scheduled and evaluated without static dispatch. [`Block::execute`](Block::execute)
/// returns an opaque future, which keeps [`Block`] itself from being a
/// trait object; this trait boxes the future instead.
#[cfg(not(target_arch = "wasm32"))]
trait ErasedBlock: BlockProps<Reader = ReaderImpl, Writer = WriterImpl> + Send + Sync {
    fn execute_boxed(&mut self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>>;
}

#[cfg(not(target_arch = "wasm32"))]
impl<B> ErasedBlock for B
where
    B: Block<Reader = ReaderImpl, Writer = WriterImpl> + Send + Sync,
{
    fn execute_boxed(&mut self) -> Pin<Box<dyn Future<Output = ()> + Send + '_>> {
        Box::pin(self.execute())
    }
}

#[cfg(target_arch = "wasm32")]
trait ErasedBlock: BlockProps<Reader = ReaderImpl, Writer = WriterImpl> {
    fn execute_boxed(&mut self) -> Pin<Box<dyn Future<Output = ()> + '_>>;
}

#[cfg(target_arch = "wasm32")]
impl<B> ErasedBlock for B
where
    B: Block<Reader = ReaderImpl, Writer = WriterImpl>,
{
    fn execute_boxed(&mut self) -> Pin<Box<dyn Future<Output = ()> + '_>> {
        Box::pin(self.execute())
    }
}

/// A runtime-registered block behind type erasure. Lets the engines and
/// evaluator treat downstream-crate blocks uniformly with built-ins.
pub(crate) struct RegisteredBlock(Box<dyn ErasedBlock>);

impl std::fmt::Debug for RegisteredBlock {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fmt.debug_tuple("RegisteredBlock")
            .field(&self.0.desc().qname())
            .finish()
    }
}

impl BlockProps for RegisteredBlock {
    type Reader = ReaderImpl;
    type Writer = WriterImpl;

    fn id(&self) -> &Uuid {
        self.0.id()
    }

    fn name(&self) -> &str {
        self.0.name()
    }

    fn desc(&self) -> &BlockDesc {
        self.0.desc()
    }

    fn state(&self) -> BlockState {
        self.0.state()
    }

    fn set_state(&mut self, state: BlockState) -> BlockState {
        self.0.set_state(state)
    }

    fn inputs(&self) -> Vec<&(dyn BlockInput<Self::Reader, Self::Writer> + Send)> {
        self.0.inputs()
    }

    fn inputs_mut(&mut self) -> Vec<&mut (dyn BlockInput<Self::Reader, Self::Writer> + Send)> {
        self.0.inputs_mut()
    }

    fn outputs(&self) -> Vec<&(dyn BlockOutput<Self::Writer> + Send)> {
        self.0.outputs()
    }

    fn outputs_mut(&mut self) -> Vec<&mut (dyn BlockOutput<Self::Writer> + Send)> {
        self.0.outputs_mut()
    }

    fn links(&self) -> Vec<(&str, Vec<&(dyn crate::base::link::Link + Send)>)> {
        self.0.links()
    }

    fn remove_link_by_id(&mut self, link_id: &Uuid) {
        self.0.remove_link_by_id(link_id)
    }

    fn remove_all_links(&mut self) {
        self.0.remove_all_links()
    }
}

impl BlockStaticDesc for RegisteredBlock {
    fn desc() -> &'static BlockDesc {
        // Same as `JsBlock`: the desc lives in the instance, there is no
        // static one. Nothing on the scheduling or eval path calls this.
        unimplemented!()
    }
}

impl Block for RegisteredBlock {
    async fn execute(&mut self) {
        self.0.execute_boxed().await
    }
}

/// Registers block types with the global block registry.
///
/// Accepts a comma-separated list of types that implement
/// [`Block`](crate::base::block::Block). Each type is registered once at
/// program start via a [`LazyLock`](std::sync::LazyLock) and is then
/// available through [`get_block`], [`schedule_block`], and
/// [`eval_static_block`].
///
/// Within the `logic-mesh` crate this macro is invoked by auto-generated
/// code from `build.rs`, which scans `src/blocks/` for
/// `#[block]`-annotated structs. Downstream crates should use
/// [`register`] to add blocks at runtime instead.
///
/// # Examples
///
/// ```ignore
/// use crate::blocks::math::add::Add;
/// use crate::blocks::misc::sine_wave::SineWave;
///
/// register_blocks!(
///     Add,
///     SineWave
/// );
/// ```
#[macro_export]
macro_rules! register_blocks {
    ( $( $block_name:ty ),* ) => {

		/// The block registry, initialized once and used for the
		/// lifetime of the program.
		static BLOCKS: LazyLock<BlockRegistry> = LazyLock::new(|| {
			let mut reg = HashMap::new();

			$(
				register_impl::<$block_name>(&mut reg)
					.expect("duplicate statically registered block");
			)*

			reg.into()
		});


		/// Constructs the block registered under `name` — with the fixed
		/// id `uuid`, if given — and hands it to `sink`, returning its id.
		///
		/// Statically compiled blocks are handed over as their concrete
		/// type; anything else falls back to the runtime registry and is
		/// handed over as a [`RegisteredBlock`]. `lib` of [`None`]
		/// searches all libraries and errors if the name is ambiguous.
		pub(crate) fn make_block_into<K: BlockSink>(
			name: &str,
			lib: Option<&str>,
			uuid: Option<uuid::Uuid>,
			sink: &mut K,
		) -> Result<uuid::Uuid> {
			if lib == Some(CORE_LIB) {
				match name {
					$(
						stringify!($block_name) => {
							let block = match uuid {
								Some(uuid) => <$block_name>::new_uuid(uuid),
								None => <$block_name>::new(),
							};
							let id = *block.id();
							sink.accept(block)?;
							return Ok(id);
						}
					)*
					_ => {}
				}
			}
			let block = make_registered(name, lib, uuid)?;
			let id = *block.id();
			sink.accept(block)?;
			Ok(id)
		}

		/// Evaluates a statically registered block by name, returning its
		/// output values.
		///
		/// Creates a temporary block instance, feeds it the given `inputs`,
		/// executes it, and returns the outputs. `lib` of [`None`] searches
		/// all libraries.
		///
		/// # Examples
		///
		/// ```
		/// # #[tokio::main(flavor = "current_thread")]
		/// # async fn main() -> Result<(), logic_mesh::Error> {
		/// use logic_mesh::{Value, blocks::registry::eval_static_block};
		///
		/// let result = eval_static_block("Add", Some("core"), vec![Value::from(1), Value::from(2)]).await?;
		/// assert_eq!(result, vec![Value::from(3)]);
		/// # Ok(())
		/// # }
		/// ```
		pub async fn eval_static_block(name: &str, lib: Option<&str>, inputs: Vec<Value>) -> Result<Vec<Value>> {
			if lib == Some(CORE_LIB) {
				match name {
					$(
						stringify!($block_name) => {
							let mut block = <$block_name>::new();
							return eval_block_impl(&mut block, inputs).await;
						}
					)*
					_ => {}
				}
			}
			eval_registered(name, lib, inputs).await
		}
    };
}

// Block imports and register_blocks! invocation are auto-generated
// by build.rs scanning for #[block] annotated structs in src/blocks/.
include!(concat!(env!("OUT_DIR"), "/block_registry.rs"));

/// Constructs block properties from the registry.
pub fn make(name: &str, lib: Option<&str>) -> Option<Box<DynBlockProps>> {
    get_block(name, lib)?.make()
}

/// Schedules a block by name on `eng`, returning its UUID.
///
/// The block must be statically registered, or registered at runtime
/// via [`register`]. `lib` of [`None`] searches all libraries and errors
/// if the name is ambiguous. Works with every engine: registered blocks
/// meet the multi-threaded engine's [`Send`] + [`Sync`] requirement.
///
/// # Examples
///
/// ```
/// use logic_mesh::{blocks::registry::schedule_block, single_threaded::SingleThreadedEngine};
///
/// let mut engine = SingleThreadedEngine::new();
/// let id = schedule_block("Add", Some("core"), &mut engine)?;
/// assert!(engine.block_handles().iter().any(|b| *b.id() == id));
/// # Ok::<(), logic_mesh::Error>(())
/// ```
pub fn schedule_block<E>(name: &str, lib: Option<&str>, eng: &mut E) -> Result<Uuid>
where
    E: Engine<Reader = ReaderImpl, Writer = WriterImpl>,
{
    make_block_into(name, lib, None, &mut ScheduleOn(eng))
}

/// Schedules a block by name with a specific UUID.
/// See [`schedule_block`] for more details.
pub fn schedule_block_with_uuid<E>(
    name: &str,
    lib: Option<&str>,
    uuid: Uuid,
    eng: &mut E,
) -> Result<Uuid>
where
    E: Engine<Reader = ReaderImpl, Writer = WriterImpl>,
{
    make_block_into(name, lib, Some(uuid), &mut ScheduleOn(eng))
}

/// Returns a block entry from the registry.
///
/// # Examples
///
/// ```
/// use logic_mesh::blocks::registry::get_block;
///
/// let entry = get_block("Add", Some("core")).expect("Add exists");
/// assert_eq!(entry.desc.name, "Add");
/// assert_eq!(entry.desc.category, "math");
/// ```
pub fn get_block(name: &str, lib: Option<&str>) -> Option<BlockEntry> {
    let reg = BLOCKS.lock().expect("Block registry is locked");
    let lib = lib.unwrap_or(CORE_LIB);

    let reg = reg.get(lib)?;
    reg.get(name).cloned()
}

/// Returns a core block entry.
pub fn get_core_block(name: &str) -> Option<BlockEntry> {
    get_block(name, Some(CORE_LIB))
}

/// Returns all block descriptions from the registry.
///
/// # Examples
///
/// ```
/// use logic_mesh::blocks::registry::list_registered_blocks;
///
/// let blocks = list_registered_blocks();
/// assert!(blocks.iter().any(|b| b.name == "Add"));
/// assert!(blocks.iter().any(|b| b.name == "Pid"));
/// ```
pub fn list_registered_blocks() -> Vec<BlockDesc> {
    let reg = BLOCKS.lock().expect("Block registry is locked");

    let mut blocks = Vec::new();
    for lib in reg.values() {
        for block in lib.values() {
            blocks.push(block.desc.clone());
        }
    }

    blocks
}

/// Registers a block description with the registry.
pub fn register_block_desc(desc: &BlockDesc) -> Result<(), RegistryError> {
    let mut reg = BLOCKS.lock().expect("Block registry is locked");

    let lib = desc.library.clone();
    let reg = reg.entry(lib).or_default();

    let name = desc.name.clone();
    if reg.contains_key(&name) {
        return Err(RegistryError::BlockAlreadyRegistered {
            library: desc.library.clone(),
            name,
        });
    }

    reg.insert(
        name.to_string(),
        BlockEntry {
            desc: desc.clone(),
            factory: None,
        },
    );

    Ok(())
}

/// Bounds a block type must meet to be registered at runtime. All
/// macro-generated blocks satisfy this. [`Send`] + [`Sync`] (native only) lets
/// registered blocks be scheduled on the multi-threaded engine.
#[cfg(not(target_arch = "wasm32"))]
pub trait RegisterableBlock:
    Block<Reader = ReaderImpl, Writer = WriterImpl> + BlockConstruct + Default + Send + Sync + 'static
{
}
#[cfg(not(target_arch = "wasm32"))]
impl<
    T: Block<Reader = ReaderImpl, Writer = WriterImpl>
        + BlockConstruct
        + Default
        + Send
        + Sync
        + 'static,
> RegisterableBlock for T
{
}

/// Marker trait for blocks that can be registered with the block registry (WASM variant).
#[cfg(target_arch = "wasm32")]
pub trait RegisterableBlock:
    Block<Reader = ReaderImpl, Writer = WriterImpl> + BlockConstruct + Default + 'static
{
}
#[cfg(target_arch = "wasm32")]
impl<T: Block<Reader = ReaderImpl, Writer = WriterImpl> + BlockConstruct + Default + 'static>
    RegisterableBlock for T
{
}

/// Registers a block type with the registry.
///
/// # Errors
///
/// Returns an error if a block with the same name is already registered in the
/// block's library — e.g. a downstream block that omits `#[library]` and
/// so defaults to the core library, colliding with a built-in.
///
/// # Panics
///
/// Panics if the block registry is already locked.
///
/// # Examples
///
/// ```
/// use logic_mesh::{
///     BlockProps, block,
///     base::block::{Block, BlockProps as _},
///     base::input::input_reader::InputReader,
///     base::output::Output,
///     blocks::{InputImpl, OutputImpl, registry},
/// };
///
/// #[block]
/// #[derive(BlockProps, Debug)]
/// #[library = "my_lib"]
/// #[category = "custom"]
/// struct MyBlock {
///     #[input(kind = "Number")]
///     input: InputImpl,
///     #[output(kind = "Number")]
///     out: OutputImpl,
/// }
///
/// impl Block for MyBlock {
///     async fn execute(&mut self) {}
/// }
///
/// registry::register::<MyBlock>()?;
/// assert!(registry::get_block("MyBlock", Some("my_lib")).is_some());
/// # Ok::<(), logic_mesh::RegistryError>(())
/// ```
pub fn register<B: RegisterableBlock>() -> Result<(), RegistryError> {
    let mut reg = BLOCKS.lock().expect("Block registry is locked");

    register_impl::<B>(&mut reg)
}

/// Instantiate a runtime-registered block by name. A qualified lookup
/// (`lib` given) resolves directly in that library; an unqualified one
/// searches all libraries and errors if the name is ambiguous.
fn make_registered(
    name: &str,
    lib: Option<&str>,
    uuid: Option<Uuid>,
) -> Result<RegisteredBlock, RegistryError> {
    // Resolve the constructor and release the lock before running it:
    // a constructor that touches the registry would otherwise deadlock,
    // and a panicking one would poison the lock for the whole process.
    let make = {
        let reg = BLOCKS.lock().expect("Block registry is locked");

        if let Some(lib) = lib {
            reg.get(lib)
                .and_then(|blocks| blocks.get(name))
                .and_then(|entry| entry.factory)
                .map(|factory| factory.make_erased)
                .ok_or_else(|| RegistryError::BlockNotFound {
                    library: lib.to_string(),
                    name: name.to_string(),
                })?
        } else {
            let matches: Vec<_> = reg
                .iter()
                .filter_map(|(lib, blocks)| {
                    blocks
                        .get(name)
                        .and_then(|entry| entry.factory)
                        .map(|factory| (lib.as_str(), factory.make_erased))
                })
                .collect();

            match matches.as_slice() {
                [] => {
                    return Err(RegistryError::BlockNotRegistered {
                        name: name.to_string(),
                    });
                }
                [(_, make)] => *make,
                _ => {
                    let mut libraries: Vec<_> =
                        matches.iter().map(|(lib, _)| lib.to_string()).collect();
                    libraries.sort_unstable();
                    return Err(RegistryError::AmbiguousBlockName {
                        name: name.to_string(),
                        libraries,
                    });
                }
            }
        }
    };

    Ok(make(uuid))
}

/// Evaluates a runtime-registered block. Fallback used by
/// [`eval_static_block`] when the name doesn't match a statically
/// compiled block.
#[doc(hidden)]
pub async fn eval_registered(
    name: &str,
    lib: Option<&str>,
    inputs: Vec<Value>,
) -> Result<Vec<Value>> {
    let mut block = make_registered(name, lib, None)?;
    eval_block_impl(&mut block, inputs).await
}

/// Evaluates `block` directly with the given `inputs`.
pub async fn eval_block_impl<B: Block<Reader = ReaderImpl, Writer = WriterImpl>>(
    block: &mut B,
    inputs: Vec<Value>,
) -> Result<Vec<Value>> {
    for (i, input) in inputs.iter().enumerate() {
        let mut input_pins = block.inputs_mut();

        if i >= input_pins.len() {
            return Err(RegistryError::TooManyInputs {
                declared: input_pins.len(),
                supplied: inputs.len(),
            }
            .into());
        }

        input_pins[i].increment_conn();
        if input_pins[i]
            .writer()
            .send((input.clone(), crate::base::Status::Ok))
            .is_ok()
            && i < inputs.len() - 1
        {
            block.read_inputs().await;
        }
    }

    block.execute().await;
    Ok(block.outputs().iter().map(|o| o.value().clone()).collect())
}

fn register_impl<B: RegisterableBlock>(reg: &mut MapType) -> Result<(), RegistryError> {
    let desc = <B as BlockStaticDesc>::desc();
    let lib = desc.library.clone();

    let lib_reg = reg.entry(lib).or_default();
    if lib_reg.contains_key(&desc.name) {
        return Err(RegistryError::BlockAlreadyRegistered {
            library: desc.library.clone(),
            name: desc.name.clone(),
        });
    }

    lib_reg.insert(desc.name.clone(), {
        let make = || -> Box<DynBlockProps> {
            let block = B::default();
            Box::new(block)
        };

        let make_erased = |uuid: Option<Uuid>| -> RegisteredBlock {
            let block = match uuid {
                Some(uuid) => B::with_uuid(uuid),
                None => B::default(),
            };
            RegisteredBlock(Box::new(block))
        };

        BlockEntry {
            desc: desc.clone(),
            factory: Some(BlockFactory { make, make_erased }),
        }
    });

    Ok(())
}

#[cfg(test)]
mod test {

    use crate::base::block::connect::connect_output;
    use crate::base::error::Error;
    use assert_matches::assert_matches;

    use super::*;

    #[test]
    fn test_registry() {
        let add = get_core_block("Add").expect("Add block not found");
        let random = get_core_block("Random").expect("Random block not found");
        let sine = get_core_block("SineWave").expect("SineWave block not found");

        assert_eq!(add.desc.name, "Add");
        assert_eq!(random.desc.name, "Random");
        assert_eq!(sine.desc.name, "SineWave");

        let mut random = random.make().unwrap();
        let mut outs = random.outputs_mut();

        let mut add = add.make().unwrap();
        let mut ins = add.inputs_mut();

        let out = outs.first_mut().unwrap();
        let input = ins.first_mut().unwrap();

        connect_output(*out, *input).unwrap();

        let mut eng = crate::single_threaded::SingleThreadedEngine::new();

        schedule_block("Add", Some("core"), &mut eng).expect("Block");

        assert!(eng.block_handles().iter().any(|b| b.desc().name == "Add"));
    }

    #[tokio::test]
    async fn test_block_eval() {
        let result =
            eval_static_block("Add", Some("core"), vec![Value::from(1), Value::from(2)]).await;

        assert_eq!(result.unwrap(), vec![Value::from(3)]);
    }

    /// Lookup failures are distinguishable by variant, so callers can
    /// tell an unqualified miss from a miss inside a named library
    /// without scraping the message.
    #[test]
    fn unknown_block_reports_a_matchable_variant() {
        let mut eng = crate::single_threaded::SingleThreadedEngine::new();

        // Unqualified: every library was searched and none matched.
        let err =
            schedule_block("NoSuchBlock", None, &mut eng).expect_err("unknown block is rejected");
        assert_matches!(
            err,
            Error::Registry(RegistryError::BlockNotRegistered { name }) if name == "NoSuchBlock"
        );

        // Qualified: the failure names the library that was searched.
        let err = schedule_block("NoSuchBlock", Some("no_such_lib"), &mut eng)
            .expect_err("unknown library is rejected");
        assert_matches!(
            err,
            Error::Registry(RegistryError::BlockNotFound { library, name })
                if library == "no_such_lib" && name == "NoSuchBlock"
        );
    }

    #[tokio::test]
    async fn eval_with_too_many_inputs_reports_the_counts() {
        // `Max` declares two inputs; supplying three overruns it.
        let err = eval_static_block(
            "Max",
            Some(CORE_LIB),
            vec![Value::from(1), Value::from(2), Value::from(3)],
        )
        .await
        .expect_err("surplus inputs should be rejected");

        assert_matches!(
            err,
            Error::Registry(RegistryError::TooManyInputs {
                declared: 2,
                supplied: 3
            })
        );
    }

    mod runtime_registered {
        use super::super::*;
        use crate::base::block::Block;
        use crate::blocks::{InputImpl, OutputImpl};

        #[block]
        #[derive(BlockProps, Debug)]
        #[library = "runtime_test"]
        #[category = "test"]
        struct Increment {
            #[input(name = "in", kind = "Number")]
            input: InputImpl,
            #[output(kind = "Number")]
            out: OutputImpl,
        }

        impl Block for Increment {
            async fn execute(&mut self) {
                use crate::base::input::InputProps;
                use crate::base::input::input_reader::InputReader;
                use crate::base::output::Output;

                self.read_inputs_until_ready().await;

                if let Some(value) = self.input.get_value()
                    && let Ok(num) = f64::try_from(value)
                {
                    self.out.set((num + 1.0).into());
                }
            }
        }

        #[tokio::test]
        async fn eval_falls_back_to_runtime_registry() {
            let _ = register::<Increment>();

            let result = eval_static_block("Increment", None, vec![Value::from(41)]).await;
            assert_eq!(result.unwrap(), vec![Value::from(42)]);
        }

        #[test]
        fn schedule_falls_back_to_runtime_registry() {
            let _ = register::<Increment>();

            let mut eng = crate::single_threaded::SingleThreadedEngine::new();
            let uuid = Uuid::new_v4();
            let id =
                schedule_block_with_uuid("Increment", None, uuid, &mut eng).expect("scheduled");
            assert_eq!(id, uuid);

            assert!(
                eng.block_handles()
                    .iter()
                    .any(|b| b.desc().name == "Increment" && b.desc().library == "runtime_test")
            );
        }

        /// A description-only entry — how JS blocks are registered —
        /// carries no factory, so it cannot be built by the registry.
        #[test]
        fn description_only_entry_has_no_factory() {
            let mut desc = <Increment as crate::base::block::BlockStaticDesc>::desc().clone();
            desc.library = "desc_only_test".to_string();
            register_block_desc(&desc).expect("registered");

            let entry = get_block("Increment", Some("desc_only_test")).expect("entry");
            assert!(entry.make().is_none());

            let mut eng = crate::single_threaded::SingleThreadedEngine::new();
            let err = schedule_block("Increment", Some("desc_only_test"), &mut eng)
                .expect_err("nothing to construct");
            assert_matches::assert_matches!(
                err,
                crate::base::error::Error::Registry(RegistryError::BlockNotFound { library, .. })
                    if library == "desc_only_test"
            );
        }

        #[tokio::test]
        async fn unknown_block_still_errors() {
            assert!(
                eval_static_block("NoSuchBlock", None, vec![])
                    .await
                    .is_err()
            );
        }

        mod lib_a {
            use super::*;

            #[block]
            #[derive(BlockProps, Debug)]
            #[library = "clash_lib_a"]
            #[category = "test"]
            pub(super) struct Clash {
                #[input(name = "in", kind = "Number")]
                input: InputImpl,
                #[output(kind = "Number")]
                out: OutputImpl,
            }

            impl Block for Clash {
                async fn execute(&mut self) {}
            }
        }

        mod lib_b {
            use super::*;

            #[block]
            #[derive(BlockProps, Debug)]
            #[library = "clash_lib_b"]
            #[category = "test"]
            pub(super) struct Clash {
                #[input(name = "in", kind = "Number")]
                input: InputImpl,
                #[output(kind = "Number")]
                out: OutputImpl,
            }

            impl Block for Clash {
                async fn execute(&mut self) {}
            }
        }

        #[test]
        fn name_clash_across_libraries_errors_with_context() {
            let _ = register::<lib_a::Clash>();
            let _ = register::<lib_b::Clash>();

            let mut eng = crate::single_threaded::SingleThreadedEngine::new();
            let err =
                schedule_block("Clash", None, &mut eng).expect_err("clash should be rejected");
            assert_eq!(
                err.to_string(),
                "Block name 'Clash' is ambiguous across libraries: 'clash_lib_a', 'clash_lib_b'"
            );
        }

        #[test]
        fn qualified_name_resolves_despite_clash() {
            let _ = register::<lib_a::Clash>();
            let _ = register::<lib_b::Clash>();

            let mut eng = crate::single_threaded::SingleThreadedEngine::new();
            let id = schedule_block("Clash", Some("clash_lib_a"), &mut eng)
                .expect("qualified name should resolve");

            assert!(
                eng.block_handles()
                    .iter()
                    .any(|b| *b.id() == id && b.desc().library == "clash_lib_a")
            );
        }

        // A downstream block whose ident collides with a built-in must be
        // reachable when qualified by its own library, not silently
        // shadowed by the static dispatch arm.
        #[block]
        #[derive(BlockProps, Debug)]
        #[library = "shadow_test"]
        #[category = "test"]
        struct Random {
            #[input(name = "in", kind = "Number")]
            input: InputImpl,
            #[output(kind = "Number")]
            out: OutputImpl,
        }

        impl Block for Random {
            async fn execute(&mut self) {
                use crate::base::output::Output;
                self.out.set(42.into());
            }
        }

        /// The `Send` requirement is enforced by the `Engine::schedule`
        /// bound, so the generic entry points schedule on the MT engine
        /// too — built-in and runtime-registered blocks alike.
        #[cfg(feature = "multi-threaded")]
        #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
        async fn generic_schedule_works_on_mt_engine() {
            let _ = register::<Increment>();

            let mut eng = crate::tokio_impl::engine::multi_threaded::MultiThreadedEngine::new();
            let add = schedule_block("Add", Some(CORE_LIB), &mut eng).expect("built-in");
            let inc = schedule_block("Increment", None, &mut eng).expect("runtime-registered");

            let ids: Vec<Uuid> = eng.block_handles().iter().map(|b| *b.id()).collect();
            assert!(ids.contains(&add) && ids.contains(&inc));
        }

        #[tokio::test]
        async fn qualified_lookup_is_not_shadowed_by_builtin() {
            let _ = register::<Random>();

            let result = eval_static_block("Random", Some("shadow_test"), vec![]).await;
            assert_eq!(result.unwrap(), vec![Value::from(42)]);
        }

        #[test]
        fn duplicate_registration_errors_with_context() {
            let _ = register::<Increment>();

            let desc = <Increment as crate::base::block::BlockStaticDesc>::desc();
            let err = register_block_desc(desc).expect_err("duplicate should be rejected");
            assert_eq!(
                err.to_string(),
                "Block 'Increment' is already registered in library 'runtime_test'"
            );
        }

        #[block]
        #[derive(BlockProps, Debug)]
        #[library = "dup_test"]
        #[category = "test"]
        struct Dup {
            #[input(name = "in", kind = "Number")]
            input: InputImpl,
            #[output(kind = "Number")]
            out: OutputImpl,
        }

        impl Block for Dup {
            async fn execute(&mut self) {}
        }

        #[test]
        fn duplicate_register_errors_instead_of_overwriting() {
            register::<Dup>().expect("first registration succeeds");

            let err = register::<Dup>().expect_err("duplicate should be rejected");
            assert_eq!(
                err.to_string(),
                "Block 'Dup' is already registered in library 'dup_test'"
            );
        }
    }
}
