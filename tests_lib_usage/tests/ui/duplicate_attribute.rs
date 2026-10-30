use logic_mesh::blocks::OutputImpl;
use logic_mesh::{BlockProps, block};

#[block]
#[derive(BlockProps, Debug)]
#[category = "test"]
#[category = "math"]
struct TwoCategories {
    #[output(kind = "Number")]
    out: OutputImpl,
}

fn main() {}
