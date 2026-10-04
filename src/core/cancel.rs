use std::sync::OnceLock;

use tokio::sync::watch;

static GLOBAL: OnceLock<Cancellation> = OnceLock::new();

#[derive(Clone, Debug)]
pub struct Cancellation {
    tx: watch::Sender<bool>,
}

impl Cancellation {
    pub fn new() -> Self {
        let (tx, _) = watch::channel(false);
        Self { tx }
    }

    pub fn cancel(&self) {
        let _ = self.tx.send(true);
    }

    pub fn is_cancelled(&self) -> bool {
        *self.tx.borrow()
    }

    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.tx.subscribe()
    }
}

impl Default for Cancellation {
    fn default() -> Self {
        Self::new()
    }
}

pub fn global() -> Cancellation {
    GLOBAL.get().cloned().unwrap_or_default()
}

pub fn init_global() -> Cancellation {
    GLOBAL.get_or_init(Cancellation::new).clone()
}

#[cfg(test)]
mod tests {
    use super::Cancellation;

    #[test]
    fn cancel_flips_flag() {
        let cancel = Cancellation::new();
        assert!(!cancel.is_cancelled());
        let mut rx = cancel.subscribe();
        cancel.cancel();
        assert!(cancel.is_cancelled());
        assert!(*rx.borrow_and_update());
    }
}
