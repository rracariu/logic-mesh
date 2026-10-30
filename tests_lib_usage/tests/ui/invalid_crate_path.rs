use logic_mesh::blocks::OutputImpl;
use logic_mesh::{BlockProps, block};

#[block]
#[derive(BlockProps, Debug)]
#[logic_mesh(krate = "logic_mesh")]
#[category = "test"]
struct BadCratePath {
    #[output(kind = "Number")]
    out: OutputImpl,
}

fn main() {}
