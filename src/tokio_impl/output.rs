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
        // Watch channels coalesce: each downstream receives the latest value;
        // there is no queue and no drop-on-full failure mode. We use
        // `send_if_modified` so writes that don't change the value don't wake
        // downstream blocks — convergent feedback loops quiesce at their
        // fixed point instead of busy-cycling forever.
        for link in &mut self.links {
            if let Some(tx) = &link.tx {
                tx.send_if_modified(|current| {
                    if *current != value {
                        *current = value.clone();
                        true
                    } else {
                        false
                    }
                });
                link.state = if tx.is_closed() {
                    LinkState::Error
                } else {
                    LinkState::Connected
                };
            }
        }
        self.value = value;
    }
}
