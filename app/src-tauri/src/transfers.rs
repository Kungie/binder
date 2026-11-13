//! Cloud copies in flight. Each one runs on its own thread with a cancel flag kept here, and reports progress
//! through an emitter at most 10 times a second (the last report always goes out).

use binder_core::{Error, Result};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const EVERY: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferState {
    Running,
    Done,
    Cancelled,
    Failed,
}

/// What `done` and `total` count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    Bytes,
    Items,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TransferEvent {
    pub id: u64,
    pub name: String,
    pub unit: Unit,
    pub done: u64,
    pub total: u64,
    pub state: TransferState,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct Transfers {
    next: AtomicU64,
    flags: Mutex<HashMap<u64, Arc<AtomicBool>>>,
}

impl Transfers {
    /// Starts `work` on a thread and returns its id. `work` gets the cancel flag and a progress callback.
    pub fn spawn<W, E>(self: &Arc<Self>, name: String, unit: Unit, emit: E, work: W) -> u64
    where
        W: FnOnce(&AtomicBool, &mut dyn FnMut(u64, u64)) -> Result<()> + Send + 'static,
        E: Fn(TransferEvent) + Send + 'static,
    {
        let id = self.next.fetch_add(1, Ordering::SeqCst) + 1;
        let flag = Arc::new(AtomicBool::new(false));
        self.flags.lock().unwrap().insert(id, flag.clone());
        let me = self.clone();
        std::thread::spawn(move || {
            let event = |done, total, state, error| TransferEvent { id, name: name.clone(), unit, done, total, state, error };
            let (mut done, mut total) = (0, 0);
            let mut last: Option<Instant> = None;
            let result = work(&flag, &mut |d, t| {
                (done, total) = (d, t);
                if last.is_none_or(|l| l.elapsed() >= EVERY) {
                    last = Some(Instant::now());
                    emit(event(d, t, TransferState::Running, None));
                }
            });
            let end = match result {
                Ok(()) => event(total, total, TransferState::Done, None),
                Err(_) if flag.load(Ordering::SeqCst) => event(done, total, TransferState::Cancelled, None),
                Err(e) => event(done, total, TransferState::Failed, Some(text(e))),
            };
            me.flags.lock().unwrap().remove(&id);
            emit(end);
        });
        id
    }

    /// Asks a transfer to stop. Unknown or finished ids are ignored.
    pub fn cancel(&self, id: u64) {
        if let Some(flag) = self.flags.lock().unwrap().get(&id) {
            flag.store(true, Ordering::SeqCst);
        }
    }
}

fn text(e: Error) -> String {
    e.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn cancel_flag_stops_a_running_transfer() {
        let transfers = Arc::new(Transfers::default());
        let (tx, rx) = mpsc::channel();
        let id = transfers.spawn("big.mov".into(), Unit::Bytes, move |e| tx.send(e).unwrap(), |cancel, progress| {
            for i in 0..1000u64 {
                if cancel.load(Ordering::SeqCst) {
                    return Err(Error::Invalid("Cancelled.".into()));
                }
                progress(i, 1000);
                std::thread::sleep(Duration::from_millis(5));
            }
            Ok(())
        });
        let first = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!((first.id, first.state), (id, TransferState::Running));
        transfers.cancel(id);
        let end = rx.iter().find(|e| e.state != TransferState::Running).unwrap();
        assert_eq!(end.state, TransferState::Cancelled);
        assert!(end.done < 1000);
        assert!(transfers.flags.lock().unwrap().is_empty());
    }

    #[test]
    fn progress_is_throttled_and_the_end_always_reported() {
        let transfers = Arc::new(Transfers::default());
        let (tx, rx) = mpsc::channel();
        transfers.spawn("a.bin".into(), Unit::Bytes, move |e| tx.send(e).unwrap(), |_, progress| {
            for i in 1..=10_000u64 {
                progress(i, 10_000);
            }
            Ok(())
        });
        let events: Vec<_> = rx.iter().take_while(|_| true).collect();
        assert!(events.len() <= 3, "{} events", events.len());
        let end = events.last().unwrap();
        assert_eq!((end.state, end.done, end.total), (TransferState::Done, 10_000, 10_000));
    }

    #[test]
    fn items_unit_is_reported() {
        let transfers = Arc::new(Transfers::default());
        let (tx, rx) = mpsc::channel();
        transfers.spawn("Archive.zip".into(), Unit::Items, move |e| tx.send(e).unwrap(), |_, progress| {
            progress(3, 41_598);
            Ok(())
        });
        let events: Vec<_> = rx.iter().collect();
        assert!(events.iter().all(|e| e.unit == Unit::Items));
        assert_eq!(serde_json::to_value(&events[0]).unwrap()["unit"], "items");
    }

    #[test]
    fn a_failed_copy_reports_its_error() {
        let transfers = Arc::new(Transfers::default());
        let (tx, rx) = mpsc::channel();
        transfers.spawn("x".into(), Unit::Bytes, move |e| tx.send(e).unwrap(), |_, _| Err(Error::Invalid("Disk full.".into())));
        let end = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!((end.state, end.error.as_deref()), (TransferState::Failed, Some("Disk full.")));
    }
}
