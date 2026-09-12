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
  type ActionSettings,
} from "./render";

const bridge = new MinibarBridge();
const bindings = new Map<string, { action: KeyAction<ActionSettings>; settings: ActionSettings }>();
let latestSnapshot: SnapshotResponse | null = null;
let refreshInFlight = false;
let refreshTimer: NodeJS.Timeout | undefined;

async function paint(binding: { action: KeyAction<ActionSettings>; settings: ActionSettings }, connected: boolean): Promise<void> {
  await binding.action.setImage(renderIndicator(latestSnapshot, binding.settings, connected), {
    target: Target.HardwareAndSoftware,
  });
  await binding.action.setTitle("");
}

async function refresh(): Promise<void> {
  if (refreshInFlight || bindings.size === 0) return;
  refreshInFlight = true;
  try {
    latestSnapshot = await bridge.snapshot();
    await Promise.all([...bindings.values()].map(binding => paint(binding, true)));
  } catch (error) {
    if (!(error instanceof BridgeUnavailableError)) streamDeck.logger.error(String(error));
    await Promise.all([...bindings.values()].map(binding => paint(binding, false)));
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
    bindings.delete(ev.action.id);
  }

  override onDidReceiveSettings(ev: DidReceiveSettingsEvent<ActionSettings>): void {
    if (!ev.action.isKey()) return;
    const binding = bindings.get(ev.action.id);
    if (!binding) return;
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
        await ev.action.setImage(renderIndicator(null, settings, false));
        await ev.action.setTitle("");
        setTimeout(() => void refresh(), 750);
      }
    }
  }
}
