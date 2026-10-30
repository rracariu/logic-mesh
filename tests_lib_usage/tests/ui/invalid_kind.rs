use logic_mesh::blocks::OutputImpl;
use logic_mesh::{BlockProps, block};

#[block]
#[derive(BlockProps, Debug)]
#[category = "test"]
struct BadKind {
    #[output(kind = "Num ber")]
    out: OutputImpl,
}

fn main() {}
