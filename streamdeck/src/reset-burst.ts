/** Fighting-game burst when a watched limit resets. Preview: streamdeck/preview/reset-burst.html */
export const RESET_BURST_DURATION_MS = 5000;
export const RESET_BURST_APPEAR_MS = 1000;
export const RESET_BURST_FRAME_MS = 33;

export interface LimitWindow {
  used_percent: number | null;
  remaining_percent: number | null;
  resets_at: string | null;
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function lerp(from: number, to: number, t: number): number {
  return from + (to - from) * t;
}

function easeOutBack(t: number): number {
  const c1 = 1.70158;
  const c3 = c1 + 1;
  return 1 + c3 * (t - 1) ** 3 + c1 * (t - 1) ** 2;
}

function easeOutCubic(t: number): number {
  return 1 - (1 - t) ** 3;
}

function unit(index: number, salt: number): number {
  const x = Math.sin(index * 127.1 + salt * 311.7) * 43758.5453;
  return x - Math.floor(x);
}

function appear(ms: number, start: number, duration: number): number {
  return clamp((ms - start) / duration, 0, 1);
}

function slam(progress: number, fromX: number): { x: number; scale: number; opacity: number } {
  if (progress <= 0) return { x: fromX, scale: 2.7, opacity: 0 };
  if (progress >= 1) return { x: 0, scale: 1, opacity: 1 };
  const eased = easeOutBack(progress);
  return {
    x: (1 - eased) * fromX,
    scale: lerp(2.7, 1, eased),
    opacity: clamp(progress * 5, 0, 1),
  };
}

function windowReset(previous: LimitWindow, next: LimitWindow): boolean {
  const remainingJump =
    previous.remaining_percent != null
    && next.remaining_percent != null
    && next.remaining_percent - previous.remaining_percent >= 35
    && next.remaining_percent >= 60;
  if (remainingJump) return true;

  const usedDrop =
    previous.used_percent != null
    && next.used_percent != null
    && previous.used_percent - next.used_percent >= 35
    && next.used_percent <= 40;
  if (usedDrop) return true;

  if (!previous.resets_at || !next.resets_at) return false;
  const jumpedForward = Date.parse(next.resets_at) - Date.parse(previous.resets_at) > 120_000;
  if (!jumpedForward) return false;
  if ((next.remaining_percent ?? 0) >= 80) return true;
  return next.used_percent != null && next.used_percent <= 20;
}

export function didLimitsReset(previous: LimitWindow[], next: LimitWindow[]): boolean {
  const count = Math.min(previous.length, next.length);
  if (count === 0) return false;
  for (let index = 0; index < count; index += 1) {
    if (windowReset(previous[index], next[index])) return true;
  }
  return false;
}

function speedLines(ms: number, opacity: number, spin: number): string {
  if (opacity <= 0.02) return "";
  const count = 20;
  let markup = "";
  for (let index = 0; index < count; index += 1) {
    const angle = (index / count) * Math.PI * 2 + spin;
    const reach = 62 + unit(index, 1) * 28;
    const inner = 10 + 7 * Math.sin(ms / 130 + index);
    const half = 1.1 + unit(index, 2) * 2.4;
    const nx = -Math.sin(angle);
    const ny = Math.cos(angle);
    const x0 = 72 + Math.cos(angle) * inner;
    const y0 = 72 + Math.sin(angle) * inner;
    const x1 = 72 + Math.cos(angle) * reach;
    const y1 = 72 + Math.sin(angle) * reach;
    const w0 = half * 0.2;
    const w1 = half;
    const fill = unit(index, 3) > 0.72 ? "#ffe600" : "#fff4c4";
    const lineOpacity = (0.1 + unit(index, 4) * 0.28) * opacity;
    markup += `<polygon points="${(x0 + nx * w0).toFixed(1)},${(y0 + ny * w0).toFixed(1)} ${(x0 - nx * w0).toFixed(1)},${(y0 - ny * w0).toFixed(1)} ${(x1 - nx * w1).toFixed(1)},${(y1 - ny * w1).toFixed(1)} ${(x1 + nx * w1).toFixed(1)},${(y1 + ny * w1).toFixed(1)}" fill="${fill}" fill-opacity="${lineOpacity.toFixed(3)}"/>`;
  }
  return markup;
}

function rings(ms: number): string {
  const hits = [720, 2180, 3360];
  return hits.map(hit => {
    const age = ms - hit;
    if (age < 0 || age > 720) return "";
    const u = age / 720;
    const radius = 8 + easeOutCubic(u) * 86;
    const width = 3.6 * (1 - u);
    const opacity = (1 - u) * 0.9;
    return `<circle cx="72" cy="72" r="${radius.toFixed(1)}" fill="none" stroke="#ffe600" stroke-width="${width.toFixed(1)}" opacity="${opacity.toFixed(2)}"/>`;
  }).join("");
}

function slash(ms: number): string {
  if (ms < 690 || ms > 1020) return "";
  const u = clamp((ms - 690) / 160, 0, 1);
  const fade = ms < 860 ? 1 : 1 - (ms - 860) / 160;
  const width = lerp(8, 170, easeOutCubic(u));
  return `<rect x="${(-width / 2).toFixed(1)}" y="-5" width="${width.toFixed(1)}" height="10" fill="#fffce8" opacity="${(0.88 * fade).toFixed(2)}" transform="translate(72 78) rotate(-18)"/>`;
}

function fightText(
  label: string,
  y: number,
  size: number,
  fill: string,
  stroke: string,
  strokeWidth: number,
  xOffset: number,
  scale: number,
  opacity: number,
  chroma: number,
): string {
  if (opacity <= 0.01 || scale <= 0.01) return "";
  const origin = `translate(72 ${y}) skewX(-12) translate(${xOffset.toFixed(1)} 0) scale(${scale.toFixed(3)})`;
  const draw = (dx: number, color: string, outline: boolean, layerOpacity: number): string => {
    const paint = outline
      ? `fill="none" stroke="${color}" stroke-width="${strokeWidth}" stroke-linejoin="round"`
      : `fill="${color}"`;
    return `<text x="${dx.toFixed(1)}" y="0" text-anchor="middle" font-family="Impact" font-size="${size}" font-weight="900" ${paint} opacity="${layerOpacity.toFixed(2)}">${label}</text>`;
  };
  const ghosts = Math.abs(xOffset) > 6
    ? `${draw(xOffset * 0.22, fill, false, 0.18)}${draw(xOffset * 0.4, stroke, false, 0.1)}`
    : "";
  const split = chroma > 0.25
    ? `${draw(-chroma, "#00e8ff", false, 0.7)}${draw(chroma, "#ff2bd6", false, 0.7)}`
    : "";
  return `<g transform="${origin}" opacity="${opacity.toFixed(3)}">${ghosts}${split}${draw(0, stroke, true, 1)}${draw(0, fill, false, 1)}</g>`;
}

function flashOpacity(ms: number): number {
  if (ms < 80) return 1;
  if (ms < 190) return 1 - (ms - 80) / 110;
  if (ms >= 700 && ms < 760) return 0.95;
  if (ms >= 760 && ms < 980) return 0.95 * (1 - (ms - 760) / 220);
  return 0;
}

export function renderResetBurstSvg(elapsedMs: number): string {
  const ms = clamp(elapsedMs, 0, RESET_BURST_DURATION_MS);
  const limitsMove = slam(appear(ms, 110, 470), -92);
  const resetMove = slam(appear(ms, 430, 570), 108);

  let shakeX = 0;
  let shakeY = 0;
  if (ms >= 680 && ms < 1280) {
    const decay = 1 - (ms - 680) / 600;
    shakeX = Math.sin(ms * 1.27) * 5.4 * decay;
    shakeY = Math.cos(ms * 1.63) * 4.1 * decay;
  } else if (ms >= 1280 && ms < 4600) {
    shakeX = Math.sin(ms * 0.09) * 0.55;
    shakeY = Math.cos(ms * 0.11) * 0.4;
  }

  const breathe = ms > RESET_BURST_APPEAR_MS
    ? 1 + 0.038 * Math.sin((ms - RESET_BURST_APPEAR_MS) / 210)
    : 1;
  const glitch = Math.sin(ms * 0.011 + 2.1);
  const chroma = ms < 1100
    ? 3.1 * appear(ms, 680, 90) * (1 - appear(ms, 820, 200))
    : glitch > 0.87 ? 3.4 : 0.65 + 0.28 * Math.sin(ms * 0.021);

  let linesOpacity = 0;
  if (ms >= 70 && ms < 420) linesOpacity = (ms - 70) / 350;
  else if (ms >= 420 && ms < 4600) linesOpacity = 0.62 + 0.38 * (0.5 + 0.5 * Math.sin(ms / 170));
  else if (ms >= 4600) linesOpacity = Math.max(0, 1 - (ms - 4600) / 280);

  let exitScale = 1;
  let exitOpacity = 1;
  if (ms > 4580) {
    const exit = (ms - 4580) / 420;
    exitScale = 1 + exit * 0.58;
    exitOpacity = 1 - exit * exit;
  }

  const limits = fightText(
    "LIMITS",
    58,
    26,
    "#f6f6f6",
    "#111111",
    6,
    limitsMove.x,
    limitsMove.scale * breathe * exitScale,
    limitsMove.opacity * exitOpacity,
    chroma * 0.55,
  );
  const reset = fightText(
    "RESET!",
    102,
    38,
    "#ffe600",
    "#d10a2e",
    7,
    resetMove.x,
    resetMove.scale * breathe * exitScale,
    resetMove.opacity * exitOpacity,
    chroma,
  );
  const flash = flashOpacity(ms);

  return `<svg xmlns="http://www.w3.org/2000/svg" width="144" height="144" viewBox="0 0 144 144"><rect width="144" height="144" fill="#050505"/><g transform="translate(${shakeX.toFixed(1)} ${shakeY.toFixed(1)})">${speedLines(ms, linesOpacity * exitOpacity, ms / 920)}${rings(ms)}${slash(ms)}${limits}${reset}</g>${flash > 0.01 ? `<rect width="144" height="144" fill="#fff" opacity="${flash.toFixed(2)}"/>` : ""}</svg>`;
}

export function renderResetBurstImage(elapsedMs: number): string {
  return `data:image/svg+xml,${encodeURIComponent(renderResetBurstSvg(elapsedMs))}`;
}
