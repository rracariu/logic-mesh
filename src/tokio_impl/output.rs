// Copyright (c) 2022-2023, Radu Racariu.

use libhaystack::val::Value;

use crate::{
    base::{
        input::InputProps,
        link::{BaseLink, LinkState},
        output::{BaseOutput, Output},
    },
    tokio_impl::WriterImpl,
};

use super::input::InputImpl;

pub type LinkImpl = BaseLink<WriterImpl>;
pub type OutputImpl = BaseOutput<LinkImpl>;

impl Output for OutputImpl {
    type Writer = <InputImpl as InputProps>::Writer;

    fn add_link(&mut self, link: BaseLink<Self::Writer>) {
        self.links.push(link);
    }

    fn set(&mut self, value: Value) {
        for link in &mut self.links {
            if let Some(tx) = &link.tx {
                if let Err(__) = tx.try_send(value.clone()) {
                    link.state = LinkState::Error;
                } else {
                    link.state = LinkState::Connected;
                }
            }
        }
        self.value = value;
    }
}
