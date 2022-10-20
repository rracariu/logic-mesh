use libhaystack::val::Value;
use tokio_impl::test_block::TestBlock;

mod base;
mod tokio_impl;

#[tokio::main]
async fn main() {
    let mut block1 = TestBlock::new();

    let mut block2 = TestBlock::new();

    block1.out.connect(&mut block2.input_a);
    block1.out.connect(&mut block2.input_b);

    block2.out.connect(&mut block1.input_a);

    block1.out.set(Value::make_int(2)).await;

    loop {
        tokio::select!(_ = block1.execute() => {}, _ = block2.execute() => {});
    }
}
