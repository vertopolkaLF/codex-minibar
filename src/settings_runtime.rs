//! Lazy WinUI lifetime. Tray and GPUI work before XAML exists; the main STA
//! thread starts Application::Start only for Settings or onboarding.

use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, OnceLock, mpsc},
};

use windows_reactor::{
    App, DispatcherQueuePriority, Element, InnerConstraints, ReactorHost, RenderCx, SendDispatcher,
    UiMarshaller, WinUIDispatcher, WindowSize,
};

type Job = Box<dyn FnOnce() + Send + 'static>;
type QueuedJob = (DispatcherQueuePriority, Job);

enum Wake {
    Work,
    Initialize,
}

enum Mode {
    Cold,
    Starting,
    Ready(UiMarshaller),
    Stopped,
}

struct RelayState {
    mode: Mode,
    pending: VecDeque<QueuedJob>,
}

struct Relay {
    state: Mutex<RelayState>,
    wake: mpsc::Sender<Wake>,
}

static RUNTIME: OnceLock<Arc<Relay>> = OnceLock::new();

impl Relay {
    fn new(wake: mpsc::Sender<Wake>) -> Self {
        Self {
            state: Mutex::new(RelayState {
                mode: Mode::Cold,
                pending: VecDeque::new(),
            }),
            wake,
        }
    }

    fn request_window(&self, job: Job) -> bool {
        let mut state = self.state.lock().unwrap();
        if let Mode::Ready(dispatcher) = &state.mode {
            let dispatcher = dispatcher.clone();
            drop(state);
            return dispatcher.dispatch(job);
        }
        if matches!(state.mode, Mode::Stopped) {
            return false;
        }
        state
            .pending
            .push_back((DispatcherQueuePriority::Normal, job));
        if matches!(state.mode, Mode::Cold) {
            // Change mode while holding the same lock as drain_cold: a queued
            // Settings job can never execute before Application::Start.
            state.mode = Mode::Starting;
            if self.wake.send(Wake::Initialize).is_err() {
                state.mode = Mode::Stopped;
                state.pending.clear();
                return false;
            }
        }
        true
    }

    fn drain_cold(&self) {
        loop {
            let job = {
                let mut state = self.state.lock().unwrap();
                if !matches!(state.mode, Mode::Cold) {
                    return;
                }
                state.pending.pop_front()
            };
            let Some((_, job)) = job else {
                return;
            };
            // Before XAML, only framework-independent jobs reach this path
            // (e.g. retaining the requested theme/accent with no root).
            job();
        }
    }

    fn attach(&self, dispatcher: UiMarshaller) {
        let mut state = self.state.lock().unwrap();
        // Queue pending work before exposing Ready, preserving FIFO order
        // against concurrent settings/provider broadcasts during bootstrap.
        while let Some((priority, job)) = state.pending.pop_front() {
            enqueue(&dispatcher, priority, job);
        }
        state.mode = Mode::Ready(dispatcher);
    }
}

fn enqueue(dispatcher: &UiMarshaller, priority: DispatcherQueuePriority, job: Job) -> bool {
    if priority == DispatcherQueuePriority::Low {
        dispatcher.dispatch_low(job)
    } else {
        dispatcher.dispatch(job)
    }
}

impl SendDispatcher for Relay {
    fn enqueue_send(&self, priority: DispatcherQueuePriority, job: Job) -> bool {
        let mut state = self.state.lock().unwrap();
        if let Mode::Ready(dispatcher) = &state.mode {
            let dispatcher = dispatcher.clone();
            drop(state);
            return enqueue(&dispatcher, priority, job);
        }
        if matches!(state.mode, Mode::Stopped) {
            return false;
        }
        state.pending.push_back((priority, job));
        if self.wake.send(Wake::Work).is_err() {
            state.mode = Mode::Stopped;
            state.pending.clear();
            return false;
        }
        true
    }
}

/// Explicit UI-opening path. Ordinary dispatches do not load XAML.
pub fn dispatch_window(job: impl FnOnce() + Send + 'static) -> bool {
    RUNTIME
        .get()
        .is_some_and(|runtime| runtime.request_window(Box::new(job)))
}

pub struct SettingsRuntime {
    relay: Arc<Relay>,
    inbox: mpsc::Receiver<Wake>,
}

impl SettingsRuntime {
    pub fn new() -> anyhow::Result<Self> {
        // COM STA + process DPI only; no Application/DispatcherQueue/XAML
        // factories or hidden WinUI host are activated here.
        windows_reactor::prepare_ui_thread()?;
        let (wake, inbox) = mpsc::channel();
        let relay = Arc::new(Relay::new(wake));
        RUNTIME
            .set(Arc::clone(&relay))
            .map_err(|_| anyhow::anyhow!("Settings runtime already installed"))?;
        Ok(Self { relay, inbox })
    }

    pub fn marshaller(&self) -> UiMarshaller {
        UiMarshaller::new(self.relay.clone())
    }

    pub fn run(self) -> anyhow::Result<()> {
        while let Ok(wake) = self.inbox.recv() {
            match wake {
                Wake::Work => self.relay.drain_cold(),
                Wake::Initialize => {
                    let relay = Arc::clone(&self.relay);
                    let result = App::new().run_custom(move |_| {
                        // Created only now. Keep the WinUI dispatcher alive
                        // across closing/reopening the last Settings window.
                        let keepalive = ReactorHost::new_with_window_options(
                            "Codex Minibar Host",
                            Some(WindowSize {
                                width: 1.0,
                                height: 1.0,
                            }),
                            InnerConstraints {
                                min_width: None,
                                min_height: None,
                                max_width: None,
                                max_height: None,
                            },
                            Box::new(|_: &(), _: &mut RenderCx| Element::Empty),
                            |_| {},
                        )?;
                        let _ = keepalive.set_shown_in_switchers(false);
                        relay.attach(WinUIDispatcher::for_current_thread()?.marshaller());
                        let _host = Box::leak(Box::new(keepalive));
                        Ok(())
                    });
                    let mut state = self.relay.state.lock().unwrap();
                    state.mode = Mode::Stopped;
                    state.pending.clear();
                    return result
                        .map_err(|error| anyhow::anyhow!("windows-reactor failed: {error:?}"));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_reactor::ChannelDispatcher;

    fn push(log: &Arc<Mutex<Vec<&'static str>>>, value: &'static str) -> Job {
        let log = Arc::clone(log);
        Box::new(move || log.lock().unwrap().push(value))
    }

    #[test]
    fn background_updates_run_without_requesting_xaml_initialization() {
        let (wake, inbox) = mpsc::channel();
        let relay = Relay::new(wake);
        let log = Arc::new(Mutex::new(Vec::new()));
        assert!(relay.enqueue_send(DispatcherQueuePriority::Normal, push(&log, "theme")));
        assert!(matches!(inbox.try_recv().unwrap(), Wake::Work));
        relay.drain_cold();
        assert_eq!(*log.lock().unwrap(), vec!["theme"]);
        assert!(matches!(relay.state.lock().unwrap().mode, Mode::Cold));
        assert!(inbox.try_recv().is_err());
    }

    #[test]
    fn settings_waits_for_xaml_and_preserves_pending_broadcast_order() {
        let (wake, inbox) = mpsc::channel();
        let relay = Relay::new(wake);
        let log = Arc::new(Mutex::new(Vec::new()));
        relay.enqueue_send(DispatcherQueuePriority::Normal, push(&log, "theme"));
        assert!(relay.request_window(push(&log, "settings")));
        assert!(relay.request_window(push(&log, "activate")));
        relay.enqueue_send(DispatcherQueuePriority::Normal, push(&log, "providers"));
        relay.drain_cold();
        assert!(log.lock().unwrap().is_empty());
        assert_eq!(
            inbox
                .try_iter()
                .filter(|wake| matches!(wake, Wake::Initialize))
                .count(),
            1
        );
        let dispatcher = ChannelDispatcher::new();
        relay.attach(dispatcher.marshaller());
        relay.enqueue_send(DispatcherQueuePriority::Normal, push(&log, "update"));
        assert!(log.lock().unwrap().is_empty());
        dispatcher.drain();
        assert_eq!(
            *log.lock().unwrap(),
            vec!["theme", "settings", "activate", "providers", "update"]
        );
    }

    #[test]
    fn unavailable_runtime_rejects_work_and_releases_captured_data() {
        let (wake, inbox) = mpsc::channel();
        let relay = Relay::new(wake);
        drop(inbox);
        let captured = Arc::new(());
        let weak = Arc::downgrade(&captured);
        assert!(!relay.enqueue_send(
            DispatcherQueuePriority::Normal,
            Box::new(move || drop(captured))
        ));
        assert!(weak.upgrade().is_none());
        assert!(matches!(relay.state.lock().unwrap().mode, Mode::Stopped));
        assert!(!relay.request_window(Box::new(|| panic!("must not run"))));
    }
}
