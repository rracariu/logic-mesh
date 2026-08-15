// Copyright (c) 2022-2023, Radu Racariu.

use libhaystack::val::Value;

use crate::{
    base::{
        Status,
        input::InputProps,
        link::BaseLink,
        output::{BaseOutput, Output},
    },
    tokio_impl::WriterImpl,
};

use super::input::InputImpl;

/// Concrete link type backed by tokio watch channels.
pub type LinkImpl = BaseLink<WriterImpl>;
/// Concrete output type backed by tokio watch channels.
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
        // fixed point instead of busy-cycling forever. Status is part of the
        // payload comparison so an externally-managed status flip (set via
        // [`Output::set_effective_status`] from the actor task) wakes
        // downstream even when the value didn't change.
        let status = self.effective_status;
        for link in &mut self.links {
            if let Some(tx) = &link.tx {
                tx.send_if_modified(|current| {
                    if current.0 != value || current.1 != status {
                        current.0 = value.clone();
                        current.1 = status;
                        true
                    } else {
                        false
                    }
                });
            }
        }
        self.value = value;
    }

    fn emit_status(&mut self, status: Status) {
        // Re-emit the current value with the new status. Updates effective
        // status so subsequent `set(value)` calls keep using it.
        self.effective_status = status;
        let value = self.value.clone();
        for link in &mut self.links {
            if let Some(tx) = &link.tx {
                tx.send_if_modified(|current| {
                    if current.0 != value || current.1 != status {
                        current.0 = value.clone();
                        current.1 = status;
                        true
                    } else {
                        false
                    }
                });
            }
        }
    }
}
