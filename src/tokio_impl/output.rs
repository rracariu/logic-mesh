// Copyright (c) 2022-2023, IntriSemantics Corp.

use tokio::sync::mpsc::Sender;

use libhaystack::val::Value;

use crate::base::{
    input::InputProps,
    link::{BaseLink, Link, LinkState},
    output::{BaseOutput, Output},
};

use super::input::InputImpl;

pub type OutputImpl = BaseOutput<BaseLink<Sender<Value>>>;

impl Output for OutputImpl {
    type Tx = <InputImpl as InputProps>::Tx;

    fn add_link(&mut self, link: BaseLink<Self::Tx>) {
        self.links.push(link);
    }

    fn remove_link(&mut self, link: &dyn Link) {
        if let Some(index) = self.links.iter().position(|l| l.id == *link.id()) {
            self.links.remove(index);
        }
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
