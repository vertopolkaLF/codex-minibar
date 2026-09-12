import type { JsonObject } from "@elgato/utils";

import type { ProviderSnapshot, SnapshotResponse, WindowSnapshot } from "./bridge";

export type ValueMode = "remaining" | "used";
export type Presentation = "numbers" | "bars" | "rings" | "reset_time" | "reset_countdown";
export type WidgetKind = "single_limit" | "dual_limit";
export type ResetDisplay = "countdown" | "time" | "hidden";
export type ClickAction = "open_popup" | "open_provider" | "cycle_provider";

export interface ActionSettings extends JsonObject {
  provider: string;
  widget: WidgetKind;
  singleMetricId: string;
  metricIds: string[];
  presentation: Presentation;
  resetDisplay: ResetDisplay;
  valueMode: ValueMode;
  showCountdown: boolean;
  clickAction: ClickAction;
  cycleProviders: string[];
  cycleIndex: number;
}

export const DEFAULT_SETTINGS: ActionSettings = {
  provider: "codex",
  widget: "single_limit",
  singleMetricId: "codex.session",
  metricIds: ["codex.session"],
  presentation: "rings",
  resetDisplay: "countdown",
  valueMode: "remaining",
  showCountdown: true,
  clickAction: "open_popup",
  cycleProviders: ["codex", "claude", "cursor"],
  cycleIndex: 0,
};

function normalizePresentation(value: unknown): Presentation {
  switch (value) {
    case "bars":
    case "rings":
    case "reset_time":
    case "reset_countdown":
      return value;
    default:
      return "numbers";
  }
}

function normalizeWidget(value: unknown): WidgetKind {
  return value === "dual_limit" ? "dual_limit" : "single_limit";
}

function normalizeResetDisplay(value: unknown): ResetDisplay {
  switch (value) {
    case "time":
    case "hidden":
      return value;
    default:
      return "countdown";
  }
}

export function normalizeSettings(settings: Partial<ActionSettings> | undefined): ActionSettings {
  const legacyMetricIds = settings?.metricIds?.filter(Boolean) ?? [];
  const widget = normalizeWidget(
    settings?.widget ?? (legacyMetricIds.length > 1 ? "dual_limit" : DEFAULT_SETTINGS.widget),
  );
  const resetDisplay = normalizeResetDisplay(
    settings?.resetDisplay
      ?? (settings?.presentation === "reset_time"
        ? "time"
        : settings?.showCountdown === false ? "hidden" : DEFAULT_SETTINGS.resetDisplay),
  );
  return {
    ...DEFAULT_SETTINGS,
    ...settings,
    widget,
    singleMetricId: settings?.singleMetricId ?? legacyMetricIds[0] ?? DEFAULT_SETTINGS.singleMetricId,
    presentation: normalizePresentation(settings?.presentation),
    resetDisplay,
    metricIds: legacyMetricIds.length ? legacyMetricIds : DEFAULT_SETTINGS.metricIds,
    cycleProviders: settings?.cycleProviders?.length ? settings.cycleProviders : DEFAULT_SETTINGS.cycleProviders,
    cycleIndex: Math.max(0, settings?.cycleIndex ?? 0),
  };
}

export function activeProvider(settings: ActionSettings): string {
  if (settings.clickAction !== "cycle_provider" || settings.cycleProviders.length === 0) {
    return settings.provider;
  }
  return settings.cycleProviders[settings.cycleIndex % settings.cycleProviders.length] ?? settings.provider;
}

interface MetricRow {
  label: string;
  window: WindowSnapshot;
}

function escapeXml(value: string): string {
  const escaped: Record<string, string> = {
    "<": "&lt;",
    ">": "&gt;",
    "&": "&amp;",
    "'": "&apos;",
    "\"": "&quot;",
  };
  return value.replace(/[<>&'\"]/g, character => escaped[character] ?? character);
}

function statusColor(remaining: number | null): string {
  if (remaining === null) return "#8490a3";
  if (remaining <= 15) return "#e64a48";
  if (remaining <= 50) return "#f59e0b";
  return "#34bc84";
}

function metricWindow(provider: ProviderSnapshot, metricId: string): MetricRow | null {
  if (metricId === `${provider.id}.primary`) return { label: "5h session", window: provider.primary };
  if (metricId === `${provider.id}.secondary`) return { label: "Weekly", window: provider.secondary };
  if (metricId.endsWith(".session")) return { label: "5h session", window: provider.primary };
  if (metricId.endsWith(".weekly")) return { label: "Weekly", window: provider.secondary };
  if (metricId === "cursor.auto") return { label: "Cursor Models", window: provider.secondary };
  const extra = provider.additional.find(item => item.metric_id === metricId);
  return extra ? { label: extra.label, window: extra.window } : null;
}

function defaultMetricIds(provider: ProviderSnapshot): string[] {
  const ids: string[] = [];
  if (provider.primary.used_percent !== null || provider.primary.resets_at !== null) {
    ids.push(`${provider.id}.session`);
  }
  if (provider.secondary.used_percent !== null || provider.secondary.resets_at !== null) {
    ids.push(`${provider.id}.weekly`);
  }
  ids.push(...provider.additional.map(item => item.metric_id));
  return ids.slice(0, 3);
}

function rowsFor(provider: ProviderSnapshot, settings: ActionSettings): MetricRow[] {
  const configuredIds = settings.widget === "single_limit"
    ? [settings.singleMetricId || settings.metricIds[0] || defaultMetricIds(provider)[0]]
    : (settings.metricIds.length ? settings.metricIds : defaultMetricIds(provider)).slice(0, 2);
  return configuredIds
    .filter((id): id is string => Boolean(id))
    .map(id => metricWindow(provider, id))
    .filter((item): item is MetricRow => item !== null)
    .slice(0, 3);
}

function displayedValue(row: MetricRow, settings: ActionSettings): number | null {
  return settings.valueMode === "used" ? row.window.used_percent : row.window.remaining_percent;
}

function formatCountdown(reset: string | null): string | null {
  if (!reset) return null;
  const seconds = Math.max(0, Math.round((Date.parse(reset) - Date.now()) / 1000));
  const days = Math.floor(seconds / 86_400);
  const hours = Math.floor((seconds % 86_400) / 3_600);
  const minutes = Math.floor((seconds % 3_600) / 60);
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
}

function countdownParts(reset: string | null): { hours: string; minutes: string } {
  if (!reset) return { hours: "?", minutes: "?" };
  const totalMinutes = Math.max(0, Math.floor((Date.parse(reset) - Date.now()) / 60_000));
  return {
    hours: String(Math.floor(totalMinutes / 60)),
    minutes: String(totalMinutes % 60).padStart(2, "0"),
  };
}

function resetCopy(reset: string | null, display: ResetDisplay): { label: string; value: string } | null {
  if (display === "hidden") return null;
  if (display === "time") {
    return { label: "reset time", value: formatResetTime(reset) };
  }
  const parts = countdownParts(reset);
  return { label: "reset in", value: `${parts.hours}:${parts.minutes}` };
}

function formatResetTime(reset: string | null): string {
  if (!reset) return "?";
  return new Date(reset).toLocaleTimeString(undefined, {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  });
}

function nearestReset(rows: MetricRow[]): string | null {
  return rows
    .map(row => row.window.resets_at)
    .filter((reset): reset is string => reset !== null)
    .sort((left, right) => Date.parse(left) - Date.parse(right))[0] ?? null;
}

function svg(body: string): string {
  return `<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144" viewBox="0 0 144 144">${body}</svg>`;
}

function renderOffline(): string {
  return svg(`<text x="72" y="88" text-anchor="middle" font-family="Segoe UI,Arial" font-size="72" font-weight="700" fill="#8490a3">?</text>`);
}

function renderNoData(): string {
  return svg(`<text x="72" y="88" text-anchor="middle" font-family="Segoe UI,Arial" font-size="62" font-weight="700" fill="#8490a3">?</text>`);
}

function renderNumbers(rows: MetricRow[], settings: ActionSettings): string {
  const fontSize = rows.length === 1 ? 72 : rows.length === 2 ? 54 : 40;
  const positions = rows.length === 1 ? [86] : rows.length === 2 ? [63, 108] : [48, 84, 120];
  const numbers = rows.map((row, index) => {
    const value = displayedValue(row, settings);
    const text = value === null ? "?" : `${value}%`;
    return `<text x="72" y="${positions[index]}" text-anchor="middle" font-family="Segoe UI,Arial" font-size="${fontSize}" font-weight="700" fill="${statusColor(row.window.remaining_percent)}">${text}</text>`;
  }).join("");
  const reset = settings.showCountdown ? formatCountdown(nearestReset(rows)) : null;
  const footer = reset ? `<text x="72" y="140" text-anchor="middle" font-family="Segoe UI,Arial" font-size="12" fill="#8490a3">${escapeXml(reset)}</text>` : "";
  return svg(`${numbers}${footer}`);
}

function renderBars(rows: MetricRow[], settings: ActionSettings): string {
  const barHeight = rows.length === 1 ? 18 : rows.length === 2 ? 14 : 10;
  const gap = rows.length === 1 ? 0 : rows.length === 2 ? 15 : 12;
  const totalHeight = rows.length * barHeight + (rows.length - 1) * gap;
  const firstY = (112 - totalHeight) / 2;
  const bars = rows.map((row, index) => {
    const value = Math.max(0, Math.min(100, displayedValue(row, settings) ?? 0));
    const y = firstY + index * (barHeight + gap);
    const color = statusColor(row.window.remaining_percent);
    return `<rect x="12" y="${y}" width="120" height="${barHeight}" rx="${barHeight / 2}" fill="#35404e"/><rect x="12" y="${y}" width="${1.2 * value}" height="${barHeight}" rx="${barHeight / 2}" fill="${color}"/>`;
  }).join("");
  const reset = settings.showCountdown ? formatCountdown(nearestReset(rows)) : null;
  const footer = reset ? `<text x="72" y="137" text-anchor="middle" font-family="Segoe UI,Arial" font-size="12" fill="#8490a3">${escapeXml(reset)}</text>` : "";
  return svg(`${bars}${footer}`);
}

function renderRings(rows: MetricRow[], settings: ActionSettings): string {
  if (rows.length === 1) return renderSingleRing(rows[0], settings);
  return renderDualRings(rows, settings);
}

function ringArc(cx: number, cy: number, radius: number, value: number, color: string, strokeWidth: number, trackColor = "#101817"): string {
  const circumference = 2 * Math.PI * radius;
  const progress = circumference * value / 100;
  return `<circle cx="${cx}" cy="${cy}" r="${radius}" fill="none" stroke="${trackColor}" stroke-width="${strokeWidth}"/><circle cx="${cx}" cy="${cy}" r="${radius}" fill="none" stroke="${color}" stroke-width="${strokeWidth}" stroke-linecap="round" stroke-dasharray="${progress} ${circumference}" transform="rotate(-90 ${cx} ${cy})"/>`;
}

/** Progress ring matching the Stream Deck key look: flat 12-o'clock start, no track, soft fade at the trailing tip. */
function fadedRingArc(cx: number, cy: number, radius: number, value: number, color: string, strokeWidth: number): string {
  const clamped = Math.max(0, Math.min(100, value));
  if (clamped <= 0) return "";

  const circumference = 2 * Math.PI * radius;
  const progressLen = circumference * clamped / 100;
  // Soft tip occupies a fixed angular span, capped so short arcs still have a solid head.
  const fadeLen = Math.min(progressLen * 0.32, circumference * 0.22, Math.max(0, progressLen - circumference * 0.08));
  const solidLen = Math.max(0, progressLen - fadeLen);
  const rotate = `transform="rotate(-90 ${cx} ${cy})"`;
  const base = `cx="${cx}" cy="${cy}" r="${radius}" fill="none" stroke="${color}" stroke-width="${strokeWidth}" stroke-linecap="butt" ${rotate}`;

  const parts: string[] = [];
  if (solidLen > 0.5) {
    parts.push(`<circle ${base} stroke-dasharray="${solidLen} ${circumference}"/>`);
  }

  const fadeSteps = 18;
  for (let i = 0; i < fadeSteps; i++) {
    const t0 = i / fadeSteps;
    const t1 = (i + 1) / fadeSteps;
    const start = solidLen + fadeLen * t0;
    const segLen = Math.max(0.75, fadeLen * (t1 - t0) + 0.6);
    // Ease-out opacity so the tip dissolves into black instead of a hard cut.
    const opacity = Math.pow(1 - t0, 1.35);
    if (opacity < 0.02) continue;
    parts.push(
      `<circle ${base} stroke-opacity="${opacity.toFixed(3)}" stroke-dasharray="${segLen.toFixed(2)} ${circumference.toFixed(2)}" stroke-dashoffset="${(-start).toFixed(2)}"/>`,
    );
  }

  return parts.join("");
}

function renderSingleRing(row: MetricRow, settings: ActionSettings): string {
  const rawValue = displayedValue(row, settings);
  const value = Math.max(0, Math.min(100, rawValue ?? 0));
  const percent = rawValue === null ? "?" : `${rawValue}%`;
  const color = statusColor(row.window.remaining_percent);
  const reset = resetCopy(row.window.resets_at, settings.resetDisplay);
  const textMarkup = reset
    ? `<text x="72" y="68" text-anchor="middle" font-family="Segoe UI,Arial" font-size="36" font-weight="700" fill="#ffffff">${escapeXml(percent)}</text><text x="72" y="84" text-anchor="middle" font-family="Segoe UI,Arial" font-size="10" fill="#9a9a9a">${reset.label}</text><text x="72" y="104" text-anchor="middle" font-family="Segoe UI,Arial" font-size="18" fill="#ececec">${escapeXml(reset.value)}</text>`
    : `<text x="72" y="84" text-anchor="middle" font-family="Segoe UI,Arial" font-size="36" font-weight="700" fill="#ffffff">${escapeXml(percent)}</text>`;
  // Stream Deck key canvas is 144x144. Stroke sits 4px in from the edge.
  const strokeWidth = 11;
  const radius = 72 - 4 - strokeWidth / 2;
  return svg(`<rect width="144" height="144" fill="#000"/>${fadedRingArc(72, 72, radius, value, color, strokeWidth)}${textMarkup}`);
}

function dualRingLabel(row: MetricRow): string {
  if (row.label === "5h session") return "5h";
  if (row.label === "Weekly") return "weekly";
  return row.label;
}

function renderDualRings(rows: MetricRow[], settings: ActionSettings): string {
  if (rows.length === 1) return renderSingleRing(rows[0], settings);
  const visibleRows = rows.slice(0, 2);
  const centers = [40, 104];
  const markup = visibleRows.map((row, index) => {
    const cx = centers[index];
    const rawValue = displayedValue(row, settings);
    const value = Math.max(0, Math.min(100, rawValue ?? 0));
    const percent = rawValue === null ? "?" : `${rawValue}%`;
    const reset = resetCopy(row.window.resets_at, settings.resetDisplay);
    const resetMarkup = reset
      ? `<text x="${cx}" y="137" text-anchor="middle" font-family="Segoe UI,Arial" font-size="9" fill="#8490a3">${escapeXml(reset.value)}</text>`
      : "";
    return `${ringArc(cx, 52, 27, value, statusColor(row.window.remaining_percent), 8)}<text x="${cx}" y="58" text-anchor="middle" font-family="Segoe UI,Arial" font-size="18" font-weight="700" fill="#f1f3f5">${escapeXml(percent)}</text><text x="${cx}" y="101" text-anchor="middle" font-family="Segoe UI,Arial" font-size="10" fill="#8490a3">${escapeXml(dualRingLabel(row))}</text>${resetMarkup}`;
  }).join("");
  return svg(`<rect width="144" height="144" fill="#000"/>${markup}`);
}

function renderResetTime(rows: MetricRow[]): string {
  const fontSize = rows.length === 1 ? 48 : rows.length === 2 ? 36 : 27;
  const positions = rows.length === 1 ? [88] : rows.length === 2 ? [66, 108] : [48, 84, 120];
  const times = rows.map((row, index) => `<text x="72" y="${positions[index]}" text-anchor="middle" font-family="Segoe UI,Arial" font-size="${fontSize}" font-weight="700" fill="${statusColor(row.window.remaining_percent)}">${formatResetTime(row.window.resets_at)}</text>`).join("");
  return svg(times);
}

function renderResetCountdown(rows: MetricRow[]): string {
  if (rows.length === 1) {
    const parts = countdownParts(rows[0].window.resets_at);
    const color = statusColor(rows[0].window.remaining_percent);
    return svg(`<text x="72" y="64" text-anchor="middle" font-family="Segoe UI,Arial" font-size="52" font-weight="700" fill="${color}">${escapeXml(parts.hours)}</text><text x="72" y="116" text-anchor="middle" font-family="Segoe UI,Arial" font-size="52" font-weight="700" fill="${color}">${escapeXml(parts.minutes)}</text>`);
  }
  const fontSize = rows.length === 2 ? 38 : 28;
  const positions = rows.length === 2 ? [66, 108] : [48, 84, 120];
  const times = rows.map((row, index) => {
    const parts = countdownParts(row.window.resets_at);
    return `<text x="72" y="${positions[index]}" text-anchor="middle" font-family="Segoe UI,Arial" font-size="${fontSize}" font-weight="700" fill="${statusColor(row.window.remaining_percent)}">${escapeXml(`${parts.hours}:${parts.minutes}`)}</text>`;
  }).join("");
  return svg(times);
}

function renderSvg(provider: ProviderSnapshot | null, settings: ActionSettings, connected: boolean): string {
  if (!connected) return renderOffline();
  if (!provider) return renderNoData();
  const rows = rowsFor(provider, settings);
  if (rows.length === 0) return renderNoData();
  return renderRings(rows, settings);
}

export function renderIndicator(snapshot: SnapshotResponse | null, settings: ActionSettings, connected: boolean): string {
  const provider = snapshot?.providers.find(item => item.id === activeProvider(settings)) ?? null;
  return `data:image/svg+xml,${encodeURIComponent(renderSvg(provider, settings, connected))}`;
}
