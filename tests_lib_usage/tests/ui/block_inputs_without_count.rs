use logic_mesh::blocks::OutputImpl;
use logic_mesh::{BlockProps, block};

#[block]
#[derive(BlockProps, Debug)]
#[category = "test"]
#[input(kind = "Number")]
struct Uncounted {
    #[output(kind = "Number")]
    out: OutputImpl,
}

fn main() {}
