//! Everything the bridge thread consumes from its single event channel.

use crate::{
    providers::instances::ProviderId, providers::status::ProviderStatusSnapshot,
    providers::worker::WorkerEvent, quota::limits::RateLimits,
    quota::reset_feed::ResetFeedSnapshot,
};

#[allow(clippy::large_enum_variant)] // moved once through a channel
#[derive(Clone, Debug)]
pub enum AppEvent {
    Provider(ProviderEvent),
    /// The bridge's single store wipe ran. Carries the clear generation and
    /// the error text on failure.
    UsageClearFinished(u64, Option<String>),
    /// Snapshot from the public Codex forced-reset announcement feed.
    ForcedResetsUpdated(ResetFeedSnapshot),
    /// Ongoing incidents from the public status pages of watched drivers.
    ProviderStatusUpdated(ProviderStatusSnapshot),
}

impl AppEvent {
    /// A quota sample obtained outside the worker loop, delivered as if that
    /// instance's worker had produced it.
    pub fn limits(provider: ProviderId, revision: u64, limits: RateLimits) -> Self {
        Self::Provider(ProviderEvent {
            provider,
            revision,
            event: WorkerEvent::LimitsUpdated(limits),
        })
    }
}

/// One worker's output tagged with the instance that emitted it.
#[derive(Clone, Debug)]
pub struct ProviderEvent {
    pub provider: ProviderId,
    /// Credential revision captured when the worker started, so the bridge
    /// can reject queued output of a replaced worker.
    pub revision: u64,
    pub event: WorkerEvent,
}
