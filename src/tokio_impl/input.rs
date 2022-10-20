use tokio::sync::mpsc::{channel, Receiver, Sender};

use libhaystack::val::Value;

use crate::base::input::{BaseInput, Input};

pub type InputImpl = BaseInput<Receiver<Value>, Sender<Value>>;

impl InputImpl {
    pub fn new() -> Self {
        let mut it = Self {
            ..Default::default()
        };

        it.init();

        it
    }
}

impl Input for InputImpl {
    type Rx = Receiver<Value>;
    type Tx = Sender<Value>;

    fn init(&mut self) {
        let (tx, rx) = channel::<Value>(32);
        self.rx = Some(rx);
        self.tx = Some(tx)
    }

    fn reader(&self) -> &Option<Self::Rx> {
        &self.rx
    }

    fn writer(&mut self) -> &mut Option<Self::Tx> {
        &mut self.tx
    }
}

#[cfg(test)]
mod test {
    use super::InputImpl;
    use crate::base::input::Input;

    #[test]
    fn test_input_init() {
        let mut input = InputImpl::default();

        assert!(input.reader().is_none());
        assert!(input.writer().is_none());

        input.init();

        assert!(input.reader().is_some());
        assert!(input.writer().is_some());
    }
}
