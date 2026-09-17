import streamDeck, {
  action,
  DidReceiveSettingsEvent,
  KeyDownEvent,
  KeyAction,
  SingletonAction,
  Target,
  WillAppearEvent,
  WillDisappearEvent,
} from "@elgato/streamdeck";

import { BridgeUnavailableError, MinibarBridge, type SnapshotResponse } from "./bridge";
import {
  activeProvider,
  DEFAULT_SETTINGS,
  normalizeSettings,
  renderIndicator,
  watchedWindows,
  type ActionSettings,
} from "./render";
import {
  RESET_BURST_DURATION_MS,
  RESET_BURST_FRAME_MS,
  didLimitsReset,
  renderResetBurstImage,
  type LimitWindow,
} from "./reset-burst";

type Binding = { action: KeyAction<ActionSettings>; settings: ActionSettings };

const bridge = new MinibarBridge();
const bindings = new Map<string, Binding>();
const previousWindows = new Map<string, LimitWindow[]>();
const bursts = new Map<string, { startedAt: number; timer: NodeJS.Timeout }>();
let latestSnapshot: SnapshotResponse | null = null;
let refreshInFlight = false;
let refreshTimer: NodeJS.Timeout | undefined;

function providerFor(settings: ActionSettings): SnapshotResponse["providers"][number] | null {
  return latestSnapshot?.providers.find(item => item.id === activeProvider(settings)) ?? null;
}

function stopBurst(id: string): void {
  const burst = bursts.get(id);
  if (!burst) return;
  clearInterval(burst.timer);
  bursts.delete(id);
}

async function paint(binding: Binding, connected: boolean): Promise<void> {
  await binding.action.setImage(renderIndicator(latestSnapshot, binding.settings, connected), {
    target: Target.HardwareAndSoftware,
  });
  await binding.action.setTitle("");
}

function startBurst(binding: Binding): void {
  stopBurst(binding.action.id);
  const startedAt = Date.now();
  const tick = (): void => {
    if (!bindings.has(binding.action.id)) {
      stopBurst(binding.action.id);
      return;
    }
    const elapsed = Date.now() - startedAt;
    if (elapsed >= RESET_BURST_DURATION_MS) {
      stopBurst(binding.action.id);
      void paint(binding, latestSnapshot !== null);
      return;
    }
    void binding.action.setImage(renderResetBurstImage(elapsed), {
      target: Target.HardwareAndSoftware,
    });
  };
  tick();
  bursts.set(binding.action.id, {
    startedAt,
    timer: setInterval(tick, RESET_BURST_FRAME_MS),
  });
}

async function applySnapshot(binding: Binding, connected: boolean): Promise<void> {
  const next = connected
    ? watchedWindows(providerFor(binding.settings), binding.settings)
    : [];
  const previous = previousWindows.get(binding.action.id);
  previousWindows.set(binding.action.id, next);
  if (!connected) {
    stopBurst(binding.action.id);
    await paint(binding, false);
    return;
  }
  if (previous && !bursts.has(binding.action.id) && didLimitsReset(previous, next)) {
    startBurst(binding);
    return;
  }
  if (bursts.has(binding.action.id)) return;
  await paint(binding, true);
}

async function refresh(): Promise<void> {
  if (refreshInFlight || bindings.size === 0) return;
  refreshInFlight = true;
  try {
    latestSnapshot = await bridge.snapshot();
    await Promise.all([...bindings.values()].map(binding => applySnapshot(binding, true)));
  } catch (error) {
    if (!(error instanceof BridgeUnavailableError)) streamDeck.logger.error(String(error));
    await Promise.all([...bindings.values()].map(binding => applySnapshot(binding, false)));
  } finally {
    refreshInFlight = false;
  }
}

function ensureRefreshLoop(): void {
  if (refreshTimer) return;
  refreshTimer = setInterval(() => void refresh(), 1000);
}

function startSettings(settings: ActionSettings | undefined): ActionSettings {
  return normalizeSettings(settings ?? DEFAULT_SETTINGS);
}

@action({ UUID: "com.vertopolkalf.codex-minibar.quota-indicator" })
export class QuotaIndicator extends SingletonAction<ActionSettings> {
  override onWillAppear(ev: WillAppearEvent<ActionSettings>): void {
    if (!ev.action.isKey()) return;
    streamDeck.logger.info(`Quota Indicator appeared: ${ev.action.id}`);
    const settings = startSettings(ev.payload.settings);
    bindings.set(ev.action.id, { action: ev.action, settings });
    ensureRefreshLoop();
    void paint({ action: ev.action, settings }, latestSnapshot !== null);
    void refresh();
  }

  override onWillDisappear(ev: WillDisappearEvent<ActionSettings>): void {
    stopBurst(ev.action.id);
    previousWindows.delete(ev.action.id);
    bindings.delete(ev.action.id);
  }

  override onDidReceiveSettings(ev: DidReceiveSettingsEvent<ActionSettings>): void {
    if (!ev.action.isKey()) return;
    const binding = bindings.get(ev.action.id);
    if (!binding) return;
    stopBurst(ev.action.id);
    previousWindows.delete(ev.action.id);
    binding.settings = startSettings(ev.payload.settings);
    streamDeck.logger.info(`Quota Indicator settings updated: ${ev.action.id} provider=${binding.settings.provider} widget=${binding.settings.widget}`);
    void paint(binding, latestSnapshot !== null);
  }

  override async onKeyDown(ev: KeyDownEvent<ActionSettings>): Promise<void> {
    streamDeck.logger.info(`Quota Indicator pressed: ${ev.action.id}`);
    const binding = bindings.get(ev.action.id);
    const settings = startSettings(ev.payload.settings ?? binding?.settings);
    try {
      if (settings.clickAction === "cycle_provider") {
        const nextIndex = settings.cycleProviders.length === 0
          ? 0
          : (settings.cycleIndex + 1) % settings.cycleProviders.length;
        const nextSettings = { ...settings, cycleIndex: nextIndex };
        stopBurst(ev.action.id);
        previousWindows.delete(ev.action.id);
        binding?.settings && (binding.settings = nextSettings);
        await ev.action.setSettings(nextSettings);
        await paint({ action: ev.action, settings: nextSettings }, latestSnapshot !== null);
        return;
      }
      await bridge.openPopup(settings.clickAction === "open_provider" ? activeProvider(settings) : undefined);
    } catch (error) {
      if (!(error instanceof BridgeUnavailableError)) streamDeck.logger.error(String(error));
      const launched = await bridge.launchMinibar();
      if (launched) {
        stopBurst(ev.action.id);
        await ev.action.setImage(renderIndicator(null, settings, false));
        await ev.action.setTitle("");
        setTimeout(() => void refresh(), 750);
      }
    }
  }
}
