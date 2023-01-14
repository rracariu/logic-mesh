use tokio::sync::mpsc::Sender;

use libhaystack::val::Value;

use crate::base::{
    input::Input,
    link::{BaseLink, LinkState},
    output::BaseOutput,
};

use super::input::InputImpl;

pub type OutputImpl = BaseOutput<BaseLink<Sender<Value>>>;

impl OutputImpl {
    pub fn connect(&mut self, input: &mut InputImpl) {
        let mut link = BaseLink::<<InputImpl as Input>::Tx>::new();

        link.tx = Some(input.writer().clone());

        link.state = LinkState::Connected;

        self.links.push(link);
    }

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
