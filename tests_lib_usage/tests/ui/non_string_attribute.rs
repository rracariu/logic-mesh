use logic_mesh::blocks::OutputImpl;
use logic_mesh::{BlockProps, block};

#[block]
#[derive(BlockProps, Debug)]
#[category = "test"]
#[ver = 2]
struct NumericVersion {
    #[output(kind = "Number")]
    out: OutputImpl,
}

fn main() {}
