use tokio::sync::mpsc::Sender;

use libhaystack::val::Value;

use crate::base::{
    input::InputProps,
    link::{BaseLink, LinkState},
    output::{BaseOutput, Output},
};

use super::input::InputImpl;

pub type OutputImpl = BaseOutput<BaseLink<Sender<Value>>>;

impl Output for OutputImpl {
    type Tx = <InputImpl as InputProps>::Tx;

    fn add_link(&mut self, link: BaseLink<Self::Tx>) {
        self.links.push(link);
    }
}

impl OutputImpl {
    pub async fn set(&mut self, value: Value) {
        for link in &mut self.links {
            if let Some(tx) = &link.tx {
                if let Err(__) = tx.send(value.clone()).await {
                    link.state = LinkState::Error;
                } else {
                    link.state = LinkState::Connected;
                }
            }
        }
    }
}
