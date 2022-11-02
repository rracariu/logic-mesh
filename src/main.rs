use futures::{future::select_all, FutureExt};
use libhaystack::val::Value;
use tokio_impl::test_add_block::TestAddBlock;

mod base;
mod tokio_impl;

#[tokio::main]
async fn main() {
    let mut block1 = TestAddBlock::new("block1");

    let mut block2 = TestAddBlock::new("block2");

    block1.out.connect(&mut block2.input_a);
    block1.out.connect(&mut block2.input_b);

    block2.out.connect(&mut block1.input_a);
    block2.out.connect(&mut block1.input_b);

    block1.out.set(Value::make_int(2)).await;

    loop {
        let blocks = [block1.execute().boxed(), block2.execute().boxed()];
        select_all(blocks).await;
    }
}
