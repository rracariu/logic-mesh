use logic_mesh::blocks::OutputImpl;
use logic_mesh::{BlockProps, block};

#[block]
#[derive(BlockProps, Debug)]
#[library = "test"]
struct Uncategorized {
    #[output(kind = "Number")]
    out: OutputImpl,
}

fn main() {}
