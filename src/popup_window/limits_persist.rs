//! Off-thread persistence of quota snapshots.
//!
//! The tray bridge thread must never wait on the process-wide store mutex:
//! usage scans hold it for seconds. Snapshots are queued here instead; the
//! writer coalesces to the latest value per provider (latest wins) and
//! writes them in the background.

use std::{
    collections::HashMap,
    sync::mpsc::{Sender, channel},
    thread,
    time::Duration,
};

use crate::{instances::ProviderId, limits::RateLimits};

enum Message {
    Save(ProviderId, Box<RateLimits>),
    Flush(Sender<()>),
}

pub(super) struct LimitsPersister {
    tx: Sender<Message>,
}

impl LimitsPersister {
    /// Spawns the writer thread. `write` performs the actual storage write.
    pub(super) fn spawn<F>(write: F) -> Self
    where
        F: Fn(ProviderId, &RateLimits) + Send + 'static,
    {
        let (tx, rx) = channel::<Message>();
        let spawned = thread::Builder::new()
            .name("limits-persist".into())
            .spawn(move || {
                while let Ok(first) = rx.recv() {
                    let mut latest = HashMap::<ProviderId, Box<RateLimits>>::new();
                    let mut flushes = Vec::new();
                    let mut next = Some(first);
                    while let Some(message) = next.take().or_else(|| rx.try_recv().ok()) {
                        match message {
                            Message::Save(provider, limits) => {
                                latest.insert(provider, limits);
                            }
                            Message::Flush(done) => flushes.push(done),
                        }
                    }
                    for (provider, limits) in &latest {
                        write(*provider, limits);
                    }
                    for done in flushes {
                        let _ = done.send(());
                    }
                }
            });
        if let Err(error) = spawned {
            eprintln!("failed to start limits persister: {error}");
        }
        Self { tx }
    }

    pub(super) fn save(&self, provider: ProviderId, limits: RateLimits) {
        let _ = self.tx.send(Message::Save(provider, Box::new(limits)));
    }

    /// Waits (bounded) until everything queued so far has been written.
    pub(super) fn flush(&self, timeout: Duration) {
        let (done_tx, done_rx) = channel();
        if self.tx.send(Message::Flush(done_tx)).is_ok() {
            let _ = done_rx.recv_timeout(timeout);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn limits(used: u8) -> RateLimits {
        let mut limits = RateLimits::default();
        limits.primary.used_percent = Some(used);
        limits
    }

    #[test]
    fn coalesces_to_latest_per_provider_while_writer_is_blocked() {
        let gate = Arc::new(Mutex::new(()));
        let writes = Arc::new(Mutex::new(Vec::<(ProviderId, Option<u8>)>::new()));
        let held = gate.lock().unwrap();
        let persister = {
            let gate = Arc::clone(&gate);
            let writes = Arc::clone(&writes);
            LimitsPersister::spawn(move |provider, limits| {
                let _wait = gate.lock().unwrap();
                writes
                    .lock()
                    .unwrap()
                    .push((provider, limits.primary.used_percent));
            })
        };
        let codex = ProviderId::primary(crate::settings::ProviderKind::Codex);
        // The first save is picked up and blocks the writer on the gate; the
        // rest pile up behind it.
        persister.save(codex, limits(1));
        thread::sleep(Duration::from_millis(100));
        for used in 2..=10 {
            persister.save(codex, limits(used));
        }
        drop(held);
        persister.flush(Duration::from_secs(5));
        let writes = writes.lock().unwrap();
        assert_eq!(writes.first().map(|w| w.1), Some(Some(1)));
        assert_eq!(writes.last().map(|w| w.1), Some(Some(10)));
        assert_eq!(writes.len(), 2, "intermediate snapshots are coalesced");
    }

    #[test]
    fn flush_waits_for_pending_writes() {
        let writes = Arc::new(Mutex::new(0_u32));
        let counter = Arc::clone(&writes);
        let persister = LimitsPersister::spawn(move |_, _| {
            thread::sleep(Duration::from_millis(50));
            *counter.lock().unwrap() += 1;
        });
        persister.save(
            ProviderId::primary(crate::settings::ProviderKind::Codex),
            limits(5),
        );
        persister.flush(Duration::from_secs(5));
        assert_eq!(*writes.lock().unwrap(), 1);
    }
}
