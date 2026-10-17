use logic_mesh::blocks::InputImpl;
use logic_mesh::{BlockProps, block};

#[block]
#[derive(BlockProps, Debug)]
#[category = "test"]
struct Sink {
    #[input(kind = "Number")]
    input: InputImpl,
}

fn main() {}
