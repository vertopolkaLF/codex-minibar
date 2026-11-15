// Popup color tokens, mirrored from src/ui/popup_window/ui/theme.rs.

export type ThemeName = "fluent" | "vercel" | "dracula" | "solarized";

export type Palette = {
  theme: ThemeName;
  dark: boolean;
  cardRadius: number;
  controlRadius: number;
  cardGap: number;
  font: string;
  textPrimary: string;
  textSecondary: string;
  textTertiary: string;
  textOnAccent: string;
  accent: string;
  cardBackground: string;
  cardStroke: string;
  subtleFill: string;
  controlFill: string;
  divider: string;
  solidBackground: string;
  footerBackground: string;
  positive: string;
  caution: string;
  critical: string;
  chromeIcon: string;
  paceMarker: string;
  intervalTick: string;
  chartGrid: string;
  windowOutline: string;
  /// Backdrop opacity of the capsule over the blurred desktop.
  capsuleOpacity: number;
};

export const SEGOE = '"Segoe UI Variable Text", "Segoe UI Variable", "Segoe UI", sans-serif';
export const GEIST = '"Geist", "Segoe UI", sans-serif';

const ink = (dark: boolean, alpha: number) =>
  dark ? `rgba(255,255,255,${alpha / 255})` : `rgba(0,0,0,${alpha / 255})`;
const rgba = (r: number, g: number, b: number, a: number) => `rgba(${r},${g},${b},${a / 255})`;

/// The orange Windows accent the reference captures were taken with.
export const ORANGE = "#FB9A44";

export const fluent = (dark: boolean, accent = ORANGE): Palette => ({
  theme: "fluent",
  dark,
  cardRadius: 10,
  controlRadius: 4,
  cardGap: 6,
  font: SEGOE,
  textPrimary: dark ? rgba(255, 255, 255, 0xff) : rgba(0, 0, 0, 0xe4),
  textSecondary: dark ? rgba(255, 255, 255, 0xc5) : rgba(0, 0, 0, 0x9e),
  textTertiary: dark ? rgba(255, 255, 255, 0x87) : rgba(0, 0, 0, 0x72),
  textOnAccent: dark ? "#000" : "#fff",
  accent,
  cardBackground: dark ? rgba(255, 255, 255, 0x0d) : rgba(255, 255, 255, 0xb3),
  cardStroke: dark ? rgba(0, 0, 0, 0x19) : rgba(0, 0, 0, 0x0f),
  subtleFill: ink(dark, dark ? 0x0f : 0x09),
  controlFill: dark ? rgba(255, 255, 255, 0x0f) : rgba(255, 255, 255, 0xb3),
  divider: ink(dark, dark ? 0x15 : 0x0f),
  solidBackground: dark ? "#202020" : "#F3F3F3",
  footerBackground: rgba(0, 0, 0, dark ? 0x24 : 0x0d),
  positive: dark ? "#6CCB5F" : "#0F7B0F",
  caution: dark ? "#FCE100" : "#9D5D00",
  critical: dark ? "#FF99A4" : "#C42B1C",
  chromeIcon: dark ? "rgb(190,190,190)" : "rgb(17,17,17)",
  paceMarker: dark ? "#fff" : "#000",
  intervalTick: rgba(0, 0, 0, 0x44),
  chartGrid: dark ? rgba(0xa8, 0x9b, 0xb8, 0x33) : rgba(0x11, 0x11, 0x11, 0x24),
  windowOutline: dark ? rgba(255, 255, 255, 0x14) : rgba(0, 0, 0, 0x12),
  capsuleOpacity: dark ? 0.82 : 0.78,
});

export const vercel = (dark: boolean): Palette => {
  const pick = (light: string, d: string) => (dark ? d : light);
  const gray = (light: number, d: number) => ink(dark, Math.round((dark ? d : light) * 255));
  const g1000 = pick("#171717", "#EDEDED");
  return {
    theme: "vercel",
    dark,
    cardRadius: 8,
    controlRadius: 6,
    cardGap: 8,
    font: GEIST,
    textPrimary: g1000,
    textSecondary: pick("#4D4D4D", "#A0A0A0"),
    textTertiary: "#8F8F8F",
    textOnAccent: pick("#fff", "#000"),
    accent: g1000,
    cardBackground: pick("#fff", "#0A0A0A"),
    cardStroke: gray(0.08, 0.14),
    subtleFill: gray(0.05, 0.07),
    controlFill: gray(0.05, 0.07),
    divider: gray(0.1, 0.13),
    solidBackground: pick("#FAFAFA", "#000"),
    footerBackground: "transparent",
    positive: pick("#107D32", "#62C073"),
    caution: pick("#AA4D00", "#FF9300"),
    critical: pick("#D8001B", "#FF565F"),
    chromeIcon: pick("#4D4D4D", "#A0A0A0"),
    paceMarker: g1000,
    intervalTick: gray(0.21, 0.24),
    chartGrid: gray(0.081, 0.09),
    windowOutline: dark ? rgba(255, 255, 255, 0x14) : rgba(0, 0, 0, 0x12),
    capsuleOpacity: 0.96,
  };
};

/// A VS Code theme mapped onto popup roles: side bar canvas, editor cards,
/// primary button accent. Values are the published Dracula colors.
export const dracula = (): Palette => ({
  ...fluent(true, "#BD93F9"),
  theme: "dracula",
  textPrimary: "#F8F8F2",
  textSecondary: "rgba(248,248,242,0.78)",
  textTertiary: "#6272A4",
  textOnAccent: "#282A36",
  accent: "#BD93F9",
  cardBackground: "#282A36",
  cardStroke: "rgba(98,114,164,0.35)",
  solidBackground: "#21222C",
  footerBackground: "rgba(0,0,0,0.18)",
  positive: "#50FA7B",
  chromeIcon: "#8B93B8",
  capsuleOpacity: 0.94,
});

export const solarized = (): Palette => ({
  ...fluent(false, "#268BD2"),
  theme: "solarized",
  textPrimary: "#073642",
  textSecondary: "#586E75",
  textTertiary: "#93A1A1",
  textOnAccent: "#FDF6E3",
  accent: "#268BD2",
  cardBackground: "#FDF6E3",
  cardStroke: "rgba(147,161,161,0.35)",
  solidBackground: "#EEE8D5",
  footerBackground: "rgba(0,0,0,0.04)",
  positive: "#859900",
  chromeIcon: "#657B83",
  capsuleOpacity: 0.95,
});

export const BRAND: Record<string, { dark: string; light: string }> = {
  codex: { dark: "rgb(128,159,255)", light: "rgb(107,132,244)" },
  claude: { dark: "rgb(217,119,87)", light: "rgb(217,119,87)" },
  cursor: { dark: "#fff", light: "rgb(51,51,51)" },
  openrouter: { dark: "rgb(200,255,0)", light: "rgb(118,36,244)" },
  opencode: { dark: "rgb(205,205,205)", light: "rgb(75,75,75)" },
  antigravity: { dark: "rgb(66,133,244)", light: "rgb(66,133,244)" },
  grok: { dark: "#fff", light: "rgb(51,51,51)" },
  kiro: { dark: "rgb(151,125,255)", light: "rgb(151,125,255)" },
};

export const brand = (provider: string, p: Palette) =>
  p.dark ? BRAND[provider].dark : BRAND[provider].light;
