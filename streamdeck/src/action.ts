import streamDeck, {
  action,
  DidReceiveSettingsEvent,
  KeyDownEvent,
  KeyAction,
  KeyUpEvent,
  SingletonAction,
  SendToPluginEvent,
  Target,
  WillAppearEvent,
  WillDisappearEvent,
} from "@elgato/streamdeck";

import { BridgeUnavailableError, MinibarBridge, type SnapshotResponse } from "./bridge";
import {
  activeProvider,
  DEFAULT_SETTINGS,
  needsMigration,
  normalizeSettings,
  renderIndicator,
  selectedProvider,
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
type PendingPress = {
  startedAt: number;
  timer: NodeJS.Timeout;
  settings: ActionSettings;
  longPressTriggered: boolean;
};

const REFRESH_HOLD_MS = 700;

const bridge = new MinibarBridge();
const bindings = new Map<string, Binding>();
const pendingPresses = new Map<string, PendingPress>();
const previousWindows = new Map<string, LimitWindow[]>();
const bursts = new Map<string, { startedAt: number; timer: NodeJS.Timeout }>();
let latestSnapshot: SnapshotResponse | null = null;
let refreshInFlight = false;
let refreshTimer: NodeJS.Timeout | undefined;

function providerFor(settings: ActionSettings): SnapshotResponse["providers"][number] | null {
  return selectedProvider(latestSnapshot, settings);
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

function cancelPendingPress(id: string): PendingPress | undefined {
  const pending = pendingPresses.get(id);
  if (!pending) return;
  clearTimeout(pending.timer);
  pendingPresses.delete(id);
  return pending;
}

async function requestManualRefresh(): Promise<void> {
  try {
    await bridge.refreshData();
    streamDeck.logger.info("Stream Deck requested a background data refresh");
  } catch (error) {
    streamDeck.logger.error(`Stream Deck background refresh failed: ${String(error)}`);
  }
}

async function runClickAction(action: KeyAction<ActionSettings>, settings: ActionSettings): Promise<void> {
  try {
    if (settings.clickAction === "cycle_provider") {
      const nextIndex = settings.cycleProviders.length === 0
        ? 0
        : (settings.cycleIndex + 1) % settings.cycleProviders.length;
      const nextSettings = { ...settings, cycleIndex: nextIndex };
      stopBurst(action.id);
      previousWindows.delete(action.id);
      const binding = bindings.get(action.id);
      if (binding) binding.settings = nextSettings;
      await action.setSettings(nextSettings);
      await paint({ action, settings: nextSettings }, latestSnapshot !== null);
      return;
    }
    await bridge.openPopup(settings.clickAction === "open_provider" ? activeProvider(settings) : undefined);
  } catch (error) {
    if (!(error instanceof BridgeUnavailableError)) streamDeck.logger.error(String(error));
    const launched = await bridge.launchMinibar();
    if (launched) {
      stopBurst(action.id);
      await action.setImage(renderIndicator(null, settings, false));
      await action.setTitle("");
      setTimeout(() => void refresh(), 750);
    }
  }
}

@action({ UUID: "com.vertopolkalf.codex-minibar.quota-indicator" })
export class QuotaIndicator extends SingletonAction<ActionSettings> {
  override async onSendToPlugin(ev: SendToPluginEvent<import("@elgato/utils").JsonValue, ActionSettings>): Promise<void> {
    if (!ev.payload || typeof ev.payload !== "object" || !("op" in ev.payload) || ev.payload.op !== "catalog") return;
    try {
      const catalog = await bridge.catalog();
      await streamDeck.ui.sendToPropertyInspector(JSON.parse(JSON.stringify(catalog)));
    } catch {
      await streamDeck.ui.sendToPropertyInspector({ type: "catalog", ok: false, providers: [] });
    }
  }

  override onWillAppear(ev: WillAppearEvent<ActionSettings>): void {
    if (!ev.action.isKey()) return;
    streamDeck.logger.info(`Quota Indicator appeared: ${ev.action.id}`);
    const settings = startSettings(ev.payload.settings);
    bindings.set(ev.action.id, { action: ev.action, settings });
    // Persist the account → instance migration so the inspector and later
    // sessions read the instance id directly.
    if (needsMigration(ev.payload.settings)) void ev.action.setSettings(settings);
    ensureRefreshLoop();
    void paint({ action: ev.action, settings }, latestSnapshot !== null);
    void refresh();
  }

  override onWillDisappear(ev: WillDisappearEvent<ActionSettings>): void {
    cancelPendingPress(ev.action.id);
    stopBurst(ev.action.id);
    previousWindows.delete(ev.action.id);
    bindings.delete(ev.action.id);
  }

  override onDidReceiveSettings(ev: DidReceiveSettingsEvent<ActionSettings>): void {
    if (!ev.action.isKey()) return;
    cancelPendingPress(ev.action.id);
    const binding = bindings.get(ev.action.id);
    if (!binding) return;
    stopBurst(ev.action.id);
    previousWindows.delete(ev.action.id);
    binding.settings = startSettings(ev.payload.settings);
    streamDeck.logger.info(`Quota Indicator settings updated: ${ev.action.id} provider=${binding.settings.provider} widget=${binding.settings.widget}`);
    void paint(binding, latestSnapshot !== null);
  }

  override onKeyDown(ev: KeyDownEvent<ActionSettings>): void {
    if (!ev.action.isKey()) return;
    const settings = startSettings(ev.payload.settings ?? bindings.get(ev.action.id)?.settings);
    cancelPendingPress(ev.action.id);
    const pending: PendingPress = {
      startedAt: Date.now(),
      settings,
      longPressTriggered: false,
      timer: setTimeout(() => {
        if (pendingPresses.get(ev.action.id) !== pending) return;
        pending.longPressTriggered = true;
        void requestManualRefresh();
      }, REFRESH_HOLD_MS),
    };
    pendingPresses.set(ev.action.id, pending);
  }

  override async onKeyUp(ev: KeyUpEvent<ActionSettings>): Promise<void> {
    const pending = cancelPendingPress(ev.action.id);
    if (!pending) return;
    if (pending.longPressTriggered || Date.now() - pending.startedAt >= REFRESH_HOLD_MS) {
      if (!pending.longPressTriggered) void requestManualRefresh();
      return;
    }
    await runClickAction(ev.action, pending.settings);
  }
}
