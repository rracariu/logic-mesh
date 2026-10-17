use logic_mesh::blocks::{InputImpl, OutputImpl};
use logic_mesh::{BlockProps, block};

#[block]
#[derive(BlockProps, Debug)]
#[category = "test"]
#[input(kind = "Number", count = 2)]
struct Shadowed {
    #[input(name = "in1", kind = "Number")]
    extra: InputImpl,
    #[output(kind = "Number")]
    out: OutputImpl,
}

fn main() {}
