use logic_mesh::blocks::{InputImpl, OutputImpl};
use logic_mesh::{BlockProps, block};

#[block]
#[derive(BlockProps, Debug)]
#[category = "test"]
struct TwoNames {
    #[input(name = "a", name = "b")]
    input: InputImpl,
    #[output(kind = "Number")]
    out: OutputImpl,
}

fn main() {}
