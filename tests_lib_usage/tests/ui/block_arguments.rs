use logic_mesh::blocks::OutputImpl;
use logic_mesh::{BlockProps, block};

#[block(crate = "logic_mesh")]
#[derive(BlockProps, Debug)]
#[category = "test"]
struct WithArgs {
    #[output(kind = "Number")]
    out: OutputImpl,
}

fn main() {}
