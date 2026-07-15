use std::{path::PathBuf, sync::mpsc::Sender, time::Duration};

use anyhow::{anyhow, Result};

use crate::{
    claude::ClaudeClient,
    codex::{first_available, CodexActivator, CodexClient},
    settings::{ProviderKind, Settings},
    usage::UsageStatistics,
    worker::{self, Activator, UsageProvider, WorkerEvent, WorkerHandle},
};

/// Starts the selected provider with a caller-owned event channel. Keeping the
/// channel stable lets Settings replace a provider worker without restarting
/// the tray bridge or any open WinUI surface.
pub fn start_selected_worker(
    settings: &Settings,
    activation_path: PathBuf,
    events: Sender<WorkerEvent>,
) -> Result<WorkerHandle> {
    match settings.provider {
        ProviderKind::Codex => {
            let executable = first_available(settings.codex_path.as_deref())?;
            Ok(worker::start_worker_with_event_sender(
                CodexClient::new(&executable),
                CodexClient::new(&executable),
                CodexActivator::new(executable),
                activation_path,
                settings.automatic_activation,
                settings.history_retention_days,
                Duration::from_secs(60),
                events,
            ))
        }
        ProviderKind::Claude => Ok(worker::start_worker_with_event_sender(
            ClaudeClient::new(),
            EmptyUsageProvider,
            NoopActivator,
            activation_path,
            false,
            settings.history_retention_days,
            Duration::from_secs(60),
            events,
        )),
    }
}

struct EmptyUsageProvider;

impl UsageProvider for EmptyUsageProvider {
    fn load_cached_usage_statistics(&mut self, _: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }

    fn refresh_usage_statistics(&mut self, _: u16) -> Result<UsageStatistics> {
        Ok(UsageStatistics::default())
    }
}

struct NoopActivator;

impl Activator for NoopActivator {
    fn activate(&mut self) -> Result<()> {
        Err(anyhow!("automatic activation is only available for Codex"))
    }
}
