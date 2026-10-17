use logic_mesh::blocks::OutputImpl;
use logic_mesh::{BlockProps, block};

#[block]
#[derive(BlockProps, Debug)]
#[category = "test"]
struct RawKind {
    #[output(kind = "r#Number")]
    out: OutputImpl,
}

fn main() {}
