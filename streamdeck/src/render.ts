import { readFileSync } from "node:fs";
import path from "node:path";
import type { JsonObject } from "@elgato/utils";

import type { ProviderSnapshot, SnapshotResponse, WindowSnapshot } from "./bridge";

export type ValueMode = "remaining" | "used";
export type Presentation = "numbers" | "bars" | "rings" | "reset_time" | "reset_countdown";
export type WidgetKind = "single_limit" | "dual_limit";
export type ResetDisplay = "countdown" | "time" | "hidden";
export type ClickAction = "open_popup" | "open_provider" | "cycle_provider";
export type KeyFont = "inter" | "segoe" | "arial" | "consolas";
export type ProviderMark = "hidden" | "text" | "logo";

export interface ActionSettings extends JsonObject {
  provider: string;
  widget: WidgetKind;
  singleMetricId: string;
  metricIds: string[];
  presentation: Presentation;
  resetDisplay: ResetDisplay;
  valueMode: ValueMode;
  showPercentSymbol: boolean;
  font: KeyFont;
  providerMark: ProviderMark;
  coloredProviderMark: boolean;
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
  showPercentSymbol: true,
  font: "inter",
  providerMark: "hidden",
  coloredProviderMark: false,
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

function normalizeFont(value: unknown): KeyFont {
  switch (value) {
    case "segoe":
    case "arial":
    case "consolas":
      return value;
    default:
      return "inter";
  }
}

function normalizeProviderMark(value: unknown, widget: WidgetKind): ProviderMark {
  if (value === "logo") return "logo";
  if (value === "text") return widget === "dual_limit" ? "logo" : "text";
  return "hidden";
}

// Stream Deck's SVG rasterizer treats font-family as a single family name, not a CSS stack.
const FONT_FAMILIES: Record<KeyFont, string> = {
  inter: "Inter",
  segoe: "Segoe UI",
  arial: "Arial",
  consolas: "Consolas",
};

function fontFamily(settings: ActionSettings): string {
  return FONT_FAMILIES[settings.font] ?? FONT_FAMILIES.inter;
}

interface LogoGlyph {
  view: number;
  d: string;
  evenodd?: boolean;
}

const PROVIDER_LOGOS: Record<string, LogoGlyph> = {
  codex: {
    view: 24,
    evenodd: true,
    d: "M8.086.457a6.105 6.105 0 013.046-.415c1.333.153 2.521.72 3.564 1.7a.117.117 0 00.107.029c1.408-.346 2.762-.224 4.061.366l.063.03.154.076c1.357.703 2.33 1.77 2.918 3.198.278.679.418 1.388.421 2.126a5.655 5.655 0 01-.18 1.631.167.167 0 00.04.155 5.982 5.982 0 011.578 2.891c.385 1.901-.01 3.615-1.183 5.14l-.182.22a6.063 6.063 0 01-2.934 1.851.162.162 0 00-.108.102c-.255.736-.511 1.364-.987 1.992-1.199 1.582-2.962 2.462-4.948 2.451-1.583-.008-2.986-.587-4.21-1.736a.145.145 0 00-.14-.032c-.518.167-1.04.191-1.604.185a5.924 5.924 0 01-2.595-.622 6.058 6.058 0 01-2.146-1.781c-.203-.269-.404-.522-.551-.821a7.74 7.74 0 01-.495-1.283 6.11 6.11 0 01-.017-3.064.166.166 0 00.008-.074.115.115 0 00-.037-.064 5.958 5.958 0 01-1.38-2.202 5.196 5.196 0 01-.333-1.589 6.915 6.915 0 01.188-2.132c.45-1.484 1.309-2.648 2.577-3.493.282-.188.55-.334.802-.438.286-.12.573-.22.861-.304a.129.129 0 00.087-.087A6.016 6.016 0 015.635 2.31C6.315 1.464 7.132.846 8.086.457zm-.804 7.85a.848.848 0 00-1.473.842l1.694 2.965-1.688 2.848a.849.849 0 001.46.864l1.94-3.272a.849.849 0 00.007-.854l-1.94-3.393zm5.446 6.24a.849.849 0 000 1.695h4.848a.849.849 0 000-1.696h-4.848z",
  },
  claude: {
    view: 256,
    d: "m50.228 170.321l50.357-28.257l.843-2.463l-.843-1.361h-2.462l-8.426-.518l-28.775-.778l-24.952-1.037l-24.175-1.296l-6.092-1.297L0 125.796l.583-3.759l5.12-3.434l7.324.648l16.202 1.101l24.304 1.685l17.629 1.037l26.118 2.722h4.148l.583-1.685l-1.426-1.037l-1.101-1.037l-25.147-17.045l-27.22-18.017l-14.258-10.37l-7.713-5.25l-3.888-4.925l-1.685-10.758l7-7.713l9.397.649l2.398.648l9.527 7.323l20.35 15.75L94.817 91.9l3.889 3.24l1.555-1.102l.195-.777l-1.75-2.917l-14.453-26.118l-15.425-26.572l-6.87-11.018l-1.814-6.61c-.648-2.723-1.102-4.991-1.102-7.778l7.972-10.823L71.42 0l10.63 1.426l4.472 3.888l6.61 15.101l10.694 23.786l16.591 32.34l4.861 9.592l2.592 8.879l.973 2.722h1.685v-1.556l1.36-18.211l2.528-22.36l2.463-28.776l.843-8.1l4.018-9.722l7.971-5.25l6.222 2.981l5.12 7.324l-.713 4.73l-3.046 19.768l-5.962 30.98l-3.889 20.739h2.268l2.593-2.593l10.499-13.934l17.628-22.036l7.778-8.749l9.073-9.657l5.833-4.601h11.018l8.1 12.055l-3.628 12.443l-11.342 14.388l-9.398 12.184l-13.48 18.147l-8.426 14.518l.778 1.166l2.01-.194l30.46-6.481l16.462-2.982l19.637-3.37l8.88 4.148l.971 4.213l-3.5 8.62l-20.998 5.184l-24.628 4.926l-36.682 8.685l-.454.324l.519.648l16.526 1.555l7.065.389h17.304l32.21 2.398l8.426 5.574l5.055 6.805l-.843 5.184l-12.962 6.611l-17.498-4.148l-40.83-9.721l-14-3.5h-1.944v1.167l11.666 11.406l21.387 19.314l26.767 24.887l1.36 6.157l-3.434 4.86l-3.63-.518l-23.526-17.693l-9.073-7.972l-20.545-17.304h-1.36v1.814l4.73 6.935l25.017 37.59l1.296 11.536l-1.814 3.76l-6.481 2.268l-7.13-1.297l-14.647-20.544l-15.1-23.138l-12.185-20.739l-1.49.843l-7.194 77.448l-3.37 3.953l-7.778 2.981l-6.48-4.925l-3.436-7.972l3.435-15.749l4.148-20.544l3.37-16.333l3.046-20.285l1.815-6.74l-.13-.454l-1.49.194l-15.295 20.999l-23.267 31.433l-18.406 19.702l-4.407 1.75l-7.648-3.954l.713-7.064l4.277-6.286l25.47-32.405l15.36-20.092l9.917-11.6l-.065-1.686h-.583L44.07 198.125l-12.055 1.555l-5.185-4.86l.648-7.972l2.463-2.593l20.35-13.999z",
  },
  cursor: {
    view: 24,
    d: "M11.503.131L1.891 5.678a.84.84 0 0 0-.42.726v11.188c0 .3.162.575.42.724l9.609 5.55a1 1 0 0 0 .998 0l9.61-5.55a.84.84 0 0 0 .42-.724V6.404a.84.84 0 0 0-.42-.726L12.497.131a1.01 1.01 0 0 0-.996 0M2.657 6.338h18.55c.263 0 .43.287.297.515L12.23 22.918c-.062.107-.229.064-.229-.06V12.335a.59.59 0 0 0-.295-.51l-9.11-5.257c-.109-.063-.064-.23.061-.23",
  },
  opencode: {
    view: 24,
    d: "M22 24H2V0h20zM17 4.8H7v14.4h10z",
  },
  openrouter: {
    view: 24,
    d: "M18.654 3.87a5.087 5.087 0 110 10.174L23.7 19.09c.64.641.187 1.737-.72 1.737H8.48a8.479 8.479 0 010-16.958h10.175zM8.479 7.26a5.087 5.087 0 100 10.176 5.087 5.087 0 000-10.175z",
  },
};

function resolvedProviderMark(settings: ActionSettings): ProviderMark {
  if (settings.providerMark === "hidden") return "hidden";
  if (settings.widget === "dual_limit" || settings.providerMark === "logo") return "logo";
  return "text";
}

function brandHex(provider: ProviderSnapshot): string {
  const rgb = provider.brand_rgb;
  if (!Array.isArray(rgb) || rgb.length < 3) return "#809fff";
  return `#${rgb.slice(0, 3).map(channel => Math.max(0, Math.min(255, Number(channel) || 0)).toString(16).padStart(2, "0")).join("")}`;
}

function providerLogo(provider: ProviderSnapshot, size: number, settings: ActionSettings): string {
  const glyph = PROVIDER_LOGOS[provider.id] ?? PROVIDER_LOGOS[provider.icon] ?? PROVIDER_LOGOS.codex;
  const scale = size / glyph.view;
  const origin = (144 - size) / 2;
  const rule = glyph.evenodd ? ` fill-rule="evenodd" clip-rule="evenodd"` : "";
  const fill = settings.coloredProviderMark ? brandHex(provider) : "#ffffff";
  const opacity = settings.coloredProviderMark ? "0.4" : "0.2";
  return `<g transform="translate(${origin} ${origin}) scale(${scale})"><path d="${glyph.d}" fill="${fill}" fill-opacity="${opacity}"${rule}/></g>`;
}

function providerNameMarkup(provider: ProviderSnapshot, settings: ActionSettings, y: number): string {
  const label = provider.name;
  const size = label.length > 10 ? 10 : 12;
  const fill = settings.coloredProviderMark ? brandHex(provider) : "#c8c8c8";
  return `<text x="72" y="${y}" text-anchor="middle" font-family="${fontFamily(settings)}" font-size="${size}" font-weight="700" fill="${fill}">${escapeXml(label)}</text>`;
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
    showPercentSymbol: settings?.showPercentSymbol !== false,
    font: normalizeFont(settings?.font),
    providerMark: normalizeProviderMark(settings?.providerMark, widget),
    coloredProviderMark: settings?.coloredProviderMark === true,
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

const DEPLETED_RING = "#e64a48";

function compactMetricId(id: string): string {
  return id.toLowerCase().replace(/[^a-z0-9]/g, "");
}

function additionalMatches(
  item: { id: string; metric_id: string },
  metricId: string,
  providerId: string,
): boolean {
  if (item.metric_id === metricId || item.id === metricId) return true;
  if (metricId === `${providerId}.additional.${item.id}`) return true;
  const wanted = compactMetricId(metricId);
  return [
    item.id,
    item.metric_id,
    `${providerId}.${item.id}`,
    `${providerId}.additional.${item.id}`,
  ].some(candidate => compactMetricId(candidate) === wanted);
}

function metricWindow(provider: ProviderSnapshot, metricId: string): MetricRow | null {
  if (metricId === `${provider.id}.primary`) return { label: "5h session", window: provider.primary };
  if (metricId === `${provider.id}.secondary`) return { label: "Weekly", window: provider.secondary };
  if (metricId.endsWith(".session")) return { label: "5h session", window: provider.primary };
  if (metricId.endsWith(".weekly")) return { label: "Weekly", window: provider.secondary };
  if (metricId === "cursor.auto") return { label: "Cursor Models", window: provider.secondary };
  const extra = provider.additional.find(item => additionalMatches(item, metricId, provider.id));
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

function formatPercent(value: number | null, settings: ActionSettings): string {
  if (value === null) return "?";
  return settings.showPercentSymbol ? `${value}%` : String(value);
}

function fittedFontSize(text: string, base: number): number {
  if (text.length >= 4) return Math.max(18, base - 6);
  if (text.length >= 3) return Math.max(20, base - 2);
  return base;
}

function formatCountdown(reset: string | null): string | null {
  const totalMinutes = remainingResetMinutes(reset);
  if (totalMinutes === null || totalMinutes <= 0) return null;
  const days = Math.floor(totalMinutes / 1_440);
  const hours = Math.floor((totalMinutes % 1_440) / 60);
  const minutes = totalMinutes % 60;
  if (days > 0) return `${days}d ${hours}h`;
  if (hours > 0) return `${hours}h ${minutes}m`;
  return `${minutes}m`;
}

function countdownParts(reset: string | null): { hours: string; minutes: string } | null {
  const totalMinutes = remainingResetMinutes(reset);
  if (totalMinutes === null || totalMinutes <= 0) return null;
  return {
    hours: String(Math.floor(totalMinutes / 60)),
    minutes: String(totalMinutes % 60).padStart(2, "0"),
  };
}

function remainingResetMinutes(reset: string | null): number | null {
  if (!reset) return null;
  const parsed = Date.parse(reset);
  if (Number.isNaN(parsed)) return null;
  return Math.floor((parsed - Date.now()) / 60_000);
}

function hasActiveReset(window: WindowSnapshot): boolean {
  if (window.duration_minutes === 0) return false;
  const remaining = remainingResetMinutes(window.resets_at);
  return remaining !== null && remaining > 0;
}

function resetCopy(window: WindowSnapshot, display: ResetDisplay): { label: string; value: string } | null {
  if (display === "hidden" || !hasActiveReset(window)) return null;
  if (display === "time") {
    return { label: "reset time", value: formatResetTime(window.resets_at) };
  }
  const parts = countdownParts(window.resets_at);
  if (!parts) return null;
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

function pluginRoot(): string {
  const dirname = (globalThis as { __dirname?: string }).__dirname
    ?? path.dirname(process.argv[1] ?? process.cwd());
  return path.join(dirname, "..");
}

let cachedMinibarLogo: string | null = null;

function minibarLogoSvg(): string {
  if (cachedMinibarLogo) return cachedMinibarLogo;
  const size = Math.round(144 * 0.9);
  const origin = (144 - size) / 2;
  try {
    const png = readFileSync(path.join(pluginRoot(), "imgs", "actions", "quota-indicator@2x.png"));
    const href = `data:image/png;base64,${png.toString("base64")}`;
    cachedMinibarLogo = `<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="144" height="144" viewBox="0 0 144 144"><rect width="144" height="144" fill="#000"/><image href="${href}" xlink:href="${href}" x="${origin}" y="${origin}" width="${size}" height="${size}" preserveAspectRatio="xMidYMid meet"/></svg>`;
  } catch {
    cachedMinibarLogo = svg(`<rect width="144" height="144" fill="#000"/>`);
  }
  return cachedMinibarLogo;
}

function renderOffline(_settings: ActionSettings): string {
  return minibarLogoSvg();
}

function renderNoData(settings: ActionSettings): string {
  return svg(`<text x="72" y="88" text-anchor="middle" font-family="${fontFamily(settings)}" font-size="62" font-weight="700" fill="#8490a3">?</text>`);
}

function renderNumbers(rows: MetricRow[], settings: ActionSettings): string {
  const fontSize = rows.length === 1 ? 72 : rows.length === 2 ? 54 : 40;
  const positions = rows.length === 1 ? [86] : rows.length === 2 ? [63, 108] : [48, 84, 120];
  const font = fontFamily(settings);
  const numbers = rows.map((row, index) => {
    const value = displayedValue(row, settings);
    const text = formatPercent(value, settings);
    return `<text x="72" y="${positions[index]}" text-anchor="middle" font-family="${font}" font-size="${fontSize}" font-weight="700" fill="${statusColor(row.window.remaining_percent)}">${text}</text>`;
  }).join("");
  const reset = settings.showCountdown ? formatCountdown(nearestReset(rows)) : null;
  const footer = reset ? `<text x="72" y="140" text-anchor="middle" font-family="${font}" font-size="12" fill="#8490a3">${escapeXml(reset)}</text>` : "";
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
  const footer = reset ? `<text x="72" y="137" text-anchor="middle" font-family="${fontFamily(settings)}" font-size="12" fill="#8490a3">${escapeXml(reset)}</text>` : "";
  return svg(`${bars}${footer}`);
}

function renderRings(rows: MetricRow[], settings: ActionSettings, provider: ProviderSnapshot): string {
  if (rows.length === 1) return renderSingleRing(rows[0], settings, provider);
  return renderDualRings(rows, settings, provider);
}

/** Progress ring: dark near 12-o'clock (zero), solid stop at the current value. */
function fadedRingArc(cx: number, cy: number, radius: number, value: number, color: string, strokeWidth: number): string {
  const track = `<circle cx="${cx}" cy="${cy}" r="${radius}" fill="none" stroke="#111" stroke-width="${strokeWidth}"/>`;
  // Missing data: track only. Actual 0%: a full red ring so depletion is obvious.
  if (value < 0) return track;
  const clamped = Math.max(0, Math.min(100, value));
  if (clamped <= 0) {
    return `${track}<circle cx="${cx}" cy="${cy}" r="${radius}" fill="none" stroke="${DEPLETED_RING}" stroke-width="${strokeWidth}"/>`;
  }
  if (clamped >= 100) {
    return `${track}<circle cx="${cx}" cy="${cy}" r="${radius}" fill="none" stroke="${color}" stroke-width="${strokeWidth}"/>`;
  }

  const circumference = 2 * Math.PI * radius;
  const progressLen = circumference * clamped / 100;
  // Fade occupies the start of the arc; keep a solid head so the stop stays readable.
  const fadeLen = Math.min(progressLen * 0.32, circumference * 0.22, Math.max(0, progressLen - circumference * 0.08));
  const solidLen = Math.max(0, progressLen - fadeLen);
  const rotate = `transform="rotate(-90 ${cx} ${cy})"`;
  const base = `cx="${cx}" cy="${cy}" r="${radius}" fill="none" stroke="${color}" stroke-width="${strokeWidth}" stroke-linecap="butt" ${rotate}`;

  const parts: string[] = [track];
  const fadeSteps = 18;
  for (let i = 0; i < fadeSteps; i++) {
    const t0 = i / fadeSteps;
    const t1 = (i + 1) / fadeSteps;
    const start = fadeLen * t0;
    const segLen = Math.max(0.75, fadeLen * (t1 - t0) + 0.6);
    // Dark at zero, ramping up toward the stop.
    const opacity = 0.12 + 0.88 * Math.pow(t0, 1.1);
    if (opacity < 0.02) continue;
    parts.push(
      `<circle ${base} stroke-opacity="${opacity.toFixed(3)}" stroke-dasharray="${segLen.toFixed(2)} ${circumference.toFixed(2)}" stroke-dashoffset="${(-start).toFixed(2)}"/>`,
    );
  }

  if (solidLen > 0.5) {
    parts.push(`<circle ${base} stroke-dasharray="${solidLen} ${circumference}" stroke-dashoffset="${(-fadeLen).toFixed(2)}"/>`);
  }

  const theta = -Math.PI / 2 + (clamped / 100) * 2 * Math.PI;
  const capX = cx + radius * Math.cos(theta);
  const capY = cy + radius * Math.sin(theta);
  parts.push(`<circle cx="${capX.toFixed(2)}" cy="${capY.toFixed(2)}" r="${(strokeWidth / 2).toFixed(2)}" fill="${color}"/>`);

  return parts.join("");
}

function renderSingleRing(row: MetricRow, settings: ActionSettings, provider: ProviderSnapshot): string {
  const rawValue = displayedValue(row, settings);
  const value = rawValue === null ? -1 : Math.max(0, Math.min(100, rawValue));
  const percent = formatPercent(rawValue, settings);
  const fontSize = fittedFontSize(percent, 36);
  const color = statusColor(row.window.remaining_percent);
  const reset = resetCopy(row.window, settings.resetDisplay);
  const mark = resolvedProviderMark(settings);
  // Stream Deck key canvas is 144x144. Stroke sits 4px in from the edge.
  const strokeWidth = 11;
  const radius = 72 - 4 - strokeWidth / 2;
  const percentY = Math.round(72 + fontSize * 0.36);
  const resetY = 72 + radius - strokeWidth / 2 - 14;
  const font = fontFamily(settings);
  const watermark = mark === "logo" ? providerLogo(provider, 101, settings) : "";
  const nameMarkup = mark === "text" ? providerNameMarkup(provider, settings, 40) : "";
  const percentMarkup = `<text x="72" y="${percentY}" text-anchor="middle" font-family="${font}" font-size="${fontSize}" font-weight="700" fill="#ffffff">${escapeXml(percent)}</text>`;
  const resetMarkup = reset
    ? `<text x="72" y="${resetY}" text-anchor="middle" font-family="${font}" font-size="21" fill="#ececec">${escapeXml(reset.value)}</text>`
    : "";
  return svg(`<rect width="144" height="144" fill="#000"/>${watermark}${fadedRingArc(72, 72, radius, value, color, strokeWidth)}${nameMarkup}${percentMarkup}${resetMarkup}`);
}

function ringValue(row: MetricRow, settings: ActionSettings): { value: number; percent: string; color: string } {
  const rawValue = displayedValue(row, settings);
  return {
    value: rawValue === null ? -1 : Math.max(0, Math.min(100, rawValue)),
    percent: formatPercent(rawValue, settings),
    color: statusColor(row.window.remaining_percent),
  };
}

function renderDualRings(rows: MetricRow[], settings: ActionSettings, provider: ProviderSnapshot): string {
  if (rows.length === 1) return renderSingleRing(rows[0], settings, provider);
  const outer = ringValue(rows[0], settings);
  const inner = ringValue(rows[1], settings);
  // Same edge inset and stroke as the single-limit key; inner ring sits one stroke + gap inside.
  const strokeWidth = 11;
  const outerRadius = 72 - 4 - strokeWidth / 2;
  const innerRadius = outerRadius - strokeWidth - 6;
  const outerSize = fittedFontSize(outer.percent, 26);
  const innerSize = fittedFontSize(inner.percent, 20);
  const font = fontFamily(settings);
  const watermark = resolvedProviderMark(settings) === "logo" ? providerLogo(provider, 101, settings) : "";
  const labels = `<text x="72" y="66" text-anchor="middle" font-family="${font}" font-size="${outerSize}" font-weight="700" fill="${outer.color}">${escapeXml(outer.percent)}</text><text x="72" y="94" text-anchor="middle" font-family="${font}" font-size="${innerSize}" font-weight="700" fill="${inner.color}">${escapeXml(inner.percent)}</text>`;
  return svg(`<rect width="144" height="144" fill="#000"/>${watermark}${fadedRingArc(72, 72, outerRadius, outer.value, outer.color, strokeWidth)}${fadedRingArc(72, 72, innerRadius, inner.value, inner.color, strokeWidth)}${labels}`);
}

function renderResetTime(rows: MetricRow[], settings: ActionSettings): string {
  const fontSize = rows.length === 1 ? 48 : rows.length === 2 ? 36 : 27;
  const positions = rows.length === 1 ? [88] : rows.length === 2 ? [66, 108] : [48, 84, 120];
  const font = fontFamily(settings);
  const times = rows.map((row, index) => `<text x="72" y="${positions[index]}" text-anchor="middle" font-family="${font}" font-size="${fontSize}" font-weight="700" fill="${statusColor(row.window.remaining_percent)}">${formatResetTime(row.window.resets_at)}</text>`).join("");
  return svg(times);
}

function renderResetCountdown(rows: MetricRow[], settings: ActionSettings): string {
  const font = fontFamily(settings);
  if (rows.length === 1) {
    const parts = countdownParts(rows[0].window.resets_at);
    if (!parts) return renderNoData(settings);
    const color = statusColor(rows[0].window.remaining_percent);
    return svg(`<text x="72" y="64" text-anchor="middle" font-family="${font}" font-size="52" font-weight="700" fill="${color}">${escapeXml(parts.hours)}</text><text x="72" y="116" text-anchor="middle" font-family="${font}" font-size="52" font-weight="700" fill="${color}">${escapeXml(parts.minutes)}</text>`);
  }
  const fontSize = rows.length === 2 ? 38 : 28;
  const positions = rows.length === 2 ? [66, 108] : [48, 84, 120];
  const times = rows.map((row, index) => {
    const parts = countdownParts(row.window.resets_at);
    if (!parts) return "";
    return `<text x="72" y="${positions[index]}" text-anchor="middle" font-family="${font}" font-size="${fontSize}" font-weight="700" fill="${statusColor(row.window.remaining_percent)}">${escapeXml(`${parts.hours}:${parts.minutes}`)}</text>`;
  }).join("");
  return svg(times);
}

function renderSvg(provider: ProviderSnapshot | null, settings: ActionSettings, connected: boolean): string {
  if (!connected) return renderOffline(settings);
  if (!provider) return renderNoData(settings);
  const rows = rowsFor(provider, settings);
  if (rows.length === 0) return renderNoData(settings);
  return renderRings(rows, settings, provider);
}

export function renderIndicator(snapshot: SnapshotResponse | null, settings: ActionSettings, connected: boolean): string {
  if (!connected) return `data:image/svg+xml,${encodeURIComponent(minibarLogoSvg())}`;
  const provider = snapshot?.providers.find(item => item.id === activeProvider(settings)) ?? null;
  return `data:image/svg+xml,${encodeURIComponent(renderSvg(provider, settings, connected))}`;
}
