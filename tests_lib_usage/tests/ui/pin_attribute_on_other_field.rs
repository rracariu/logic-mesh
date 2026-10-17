use logic_mesh::blocks::OutputImpl;
use logic_mesh::{BlockProps, block};

#[block]
#[derive(BlockProps, Debug)]
#[category = "test"]
struct NotAnInput {
    #[input(kind = "Number")]
    gain: f64,
    #[output(kind = "Number")]
    out: OutputImpl,
}

fn main() {}
