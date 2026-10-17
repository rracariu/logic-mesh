use logic_mesh::blocks::{InputImpl, OutputImpl};
use logic_mesh::{BlockProps, block};

#[block]
#[derive(BlockProps, Debug)]
#[category = "test"]
struct Misspelled {
    #[input(nme = "in", kind = "Number")]
    input: InputImpl,
    #[output(kind = "Number")]
    out: OutputImpl,
}

fn main() {}
