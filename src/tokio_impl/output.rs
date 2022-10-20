use tokio::sync::mpsc::Sender;

use libhaystack::val::Value;

use crate::base::{
    input::Input,
    link::{Link, LinkState},
    output::{BaseOutput, Output},
};

pub type OutputImpl = BaseOutput<Link<Sender<Value>>>;

impl Output for OutputImpl {}

impl OutputImpl {
    pub fn connect<In: Input<Tx = Sender<Value>>>(&mut self, input: &mut In) {
        let mut link = Link::<In::Tx>::new();

        link.tx = input.writer().clone();

        link.state = if link.tx.is_some() {
            LinkState::Connected
        } else {
            LinkState::Disconnected
        };

        self.links.push(link);
    }

    pub async fn set(&mut self, value: Value) {
        for link in &mut self.links {
            if let Some(tx) = &link.tx {
                if let Err(__) = tx.send(value.clone()).await {
                    link.state = LinkState::Error;
                }
            }
        }
    }
}

#[allow(clippy::derivable_impls)]
impl Default for OutputImpl {
    fn default() -> Self {
        Self {
            links: Default::default(),
        }
    }
}
