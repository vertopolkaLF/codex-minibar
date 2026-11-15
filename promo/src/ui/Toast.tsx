// In-app notifications, mirrored from src/ui/popup_window/ui/toast.rs.
import React from "react";
import { Palette } from "./palette";
import { Icon, ProviderMark, T, cardStyle } from "./kit";
import { Quota, Strings } from "../data";

export const CARD_WIDTH = 360;
export const EDGE = 16;
export const GAP = 8;
const PADDING = 12;
const LINE = 20;
const SECTION_GAP = 12;
const LIMIT_CARD = 44;
export const ENTER = 0.36;
export const EXIT = 0.22;
const FILL_DELAY = 0.2;
const FILL = 1.1;
export const SETTLE = 0.42;

export const easeOutCubic = (t: number) => 1 - Math.pow(1 - Math.min(1, Math.max(0, t)), 3);

export type Kind = "info" | "success" | "reset" | "warning" | "error" | "update";

export type Alert = {
  provider: string;
  name: string;
  plan?: string;
  account?: string;
  badge?: { text: string; color: string };
  rows: { q: Quota; focused: boolean }[];
};

export type ToastSpec = {
  id: string;
  kind: Kind;
  title: string;
  body?: string;
  bodyLines?: number;
  actions?: string[];
  /// 0..1 fraction, "sweep" for an unknown total.
  progress?: number | "sweep";
  alert?: Alert;
};

export const kindStyle = (kind: Kind, p: Palette): [string, string] => {
  switch (kind) {
    case "success":
      return ["fluent-checkmark-circle-16-filled", p.dark ? "#6CCB5F" : "#0F7B0F"];
    case "reset":
      return ["fluent-arrow-sync-20-filled", p.accent];
    case "warning":
      return ["fluent-warning-16-filled", p.caution];
    case "error":
      return ["ph-warning-fill", p.critical];
    case "update":
      return ["fluent-arrow-download-24-filled", p.accent];
    default:
      return ["fluent-info-16-filled", p.accent];
  }
};

/// Card height from the same metrics toast.rs measures with.
export const measure = (t: ToastSpec, p: Palette) => {
  let h = 2 + PADDING * 2 + LINE;
  if (!t.alert && t.body) h += 2 + (t.bodyLines ?? 1) * LINE;
  if (t.actions?.length) h += SECTION_GAP + 28;
  if (t.progress !== undefined) h += 10 + 6;
  if (t.alert) h += SECTION_GAP + LINE + t.alert.rows.length * (p.cardGap + LIMIT_CARD);
  return h;
};

const LimitCard: React.FC<{
  q: Quota;
  focused: boolean;
  kind: Kind;
  age: number;
  p: Palette;
  s: Strings;
}> = ({ q, focused, kind, age, p, s }) => {
  const fill = focused ? easeOutCubic((age - FILL_DELAY) / FILL) : 1;
  const value = q.left * fill;
  const color = focused && kind === "warning" ? p.caution : p.accent;
  const title = (q.title === "session" ? s.session : q.title === "weekly" ? s.weekly : q.title).toUpperCase();
  return (
    <div style={{ ...cardStyle(p), height: LIMIT_CARD }}>
      <div style={{ position: "absolute", inset: 0, clipPath: `inset(0 ${100 - value}% 0 0)` }}>
        <div
          style={{
            position: "absolute",
            inset: 0,
            borderRadius: p.cardRadius - 1,
            background: `color-mix(in srgb, ${color} 20%, transparent)`,
          }}
        />
      </div>
      <div style={{ position: "absolute", inset: 0, display: "flex" }}>
        {Array.from({ length: q.ticks + 1 }, (_, i) => (
          <div key={i} style={{ flex: 1, display: "flex", justifyContent: "flex-end", alignItems: "flex-end" }}>
            {i < q.ticks && (
              <div style={{ width: 2, height: 6, borderRadius: "1px 1px 0 0", background: p.intervalTick }} />
            )}
          </div>
        ))}
      </div>
      <div
        style={{
          position: "relative",
          height: "100%",
          padding: "0 12px",
          display: "flex",
          alignItems: "center",
          gap: 10,
        }}
      >
        <T size={12} color={p.textSecondary} style={{ flex: 1 }}>
          {title}
        </T>
        <T size={14} color={color} weight={600}>
          {s.left(Math.round(value))}
        </T>
        <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
          <Icon name="fluent-arrow-clockwise-dashes-16-regular" size={14} color={p.textSecondary} />
          <T size={14} color={p.textPrimary}>
            {q.reset}
          </T>
        </div>
      </div>
    </div>
  );
};

const ProgressBar: React.FC<{ value: number | "sweep"; age: number; p: Palette }> = ({ value, age, p }) => {
  let left = 0;
  let width = typeof value === "number" ? value : 0;
  if (value === "sweep") {
    const cycle = (age / 1.6) % 1;
    // cubic-bezier(0.65, 0, 0.35, 1), approximated by smootherstep.
    const e = cycle * cycle * cycle * (cycle * (cycle * 6 - 15) + 10);
    const start = e * 1.35 - 0.35;
    left = Math.max(0, start);
    width = Math.min(1, start + 0.35) - left;
  }
  return (
    <div
      style={{
        position: "relative",
        height: 6,
        marginTop: 10,
        borderRadius: 3,
        background: `color-mix(in srgb, ${p.accent} 20%, transparent)`,
      }}
    >
      {width > 0 && (
        <div
          style={{
            position: "absolute",
            top: 0,
            bottom: 0,
            left: `${left * 100}%`,
            width: `${width * 100}%`,
            minWidth: 6,
            borderRadius: 3,
            background: p.accent,
          }}
        />
      )}
    </div>
  );
};

export const ToastCard: React.FC<{
  t: ToastSpec;
  p: Palette;
  s: Strings;
  /// Seconds since the card appeared.
  age: number;
  style?: React.CSSProperties;
}> = ({ t, p, s, age, style }) => {
  const [glyph, tint] = kindStyle(t.kind, p);
  return (
    <div
      style={{
        position: "absolute",
        width: CARD_WIDTH,
        height: measure(t, p),
        padding: PADDING,
        boxSizing: "border-box",
        borderRadius: 8,
        background: p.solidBackground,
        border: `1px solid ${p.windowOutline}`,
        boxShadow: `0 4px 12px rgba(0,0,0,${p.dark ? 0.3 : 0.14})`,
        fontFamily: p.font,
        color: p.textPrimary,
        ...style,
      }}
    >
      <div style={{ display: "flex", alignItems: "flex-start", paddingLeft: 4, gap: 8 }}>
        <div style={{ paddingTop: 2 }}>
          <Icon name={glyph} size={16} color={tint} />
        </div>
        <div style={{ flex: 1, minWidth: 0, paddingRight: 22 }}>
          <T size={14} color={p.textPrimary} weight={600}>
            {t.title}
          </T>
          {!t.alert && t.body && (
            <T size={14} color={p.textSecondary} style={{ marginTop: 2, whiteSpace: "normal" }}>
              {t.body}
            </T>
          )}
          {t.actions?.length ? (
            <div style={{ marginTop: SECTION_GAP, display: "flex", gap: 8 }}>
              {t.actions.map((label, i) => (
                <div
                  key={label}
                  style={{
                    height: 28,
                    padding: "0 12px",
                    display: "flex",
                    alignItems: "center",
                    borderRadius: 4,
                    boxSizing: "border-box",
                    background: i === 0 ? p.accent : p.controlFill,
                    border: i === 0 ? "none" : `1px solid ${p.cardStroke}`,
                    color: i === 0 ? p.textOnAccent : p.textPrimary,
                    fontSize: 14,
                    fontWeight: 600,
                  }}
                >
                  {label}
                </div>
              ))}
            </div>
          ) : null}
          {t.progress !== undefined && <ProgressBar value={t.progress} age={age} p={p} />}
        </div>
      </div>
      {t.alert && (
        <div style={{ marginTop: SECTION_GAP, display: "flex", flexDirection: "column", gap: p.cardGap }}>
          <div style={{ height: LINE, padding: "0 4px", display: "flex", alignItems: "center" }}>
            <ProviderMark provider={t.alert.provider} size={16} p={p} badge={t.alert.badge} />
            <div style={{ width: t.alert.badge ? 14 : 8 }} />
            <T size={14} color={p.textSecondary} weight={600}>
              {t.alert.name}
            </T>
            {t.alert.plan && (
              <T size={14} color={p.textTertiary} style={{ marginLeft: 4 }}>
                {t.alert.plan}
              </T>
            )}
            <div style={{ flex: 1 }} />
            {t.alert.account && (
              <T size={14} color={p.textTertiary}>
                {t.alert.account}
              </T>
            )}
          </div>
          {t.alert.rows.map(({ q, focused }) => (
            <LimitCard key={q.id} q={q} focused={focused} kind={t.kind} age={age} p={p} s={s} />
          ))}
        </div>
      )}
      <div
        style={{
          position: "absolute",
          top: PADDING - 2,
          right: PADDING - 6,
          width: 24,
          height: 24,
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
        }}
      >
        <Icon name="fluent-dismiss-16-regular" size={12} color={p.chromeIcon} />
      </div>
    </div>
  );
};

export type Timed = { spec: ToastSpec; at: number; leave?: number; replaces?: { at: number; spec: ToastSpec }[] };

/// The toast host: newest card at the bottom pushes older ones up; each
/// glides to its slot over SETTLE and leaves by sliding right.
export const ToastStack: React.FC<{
  toasts: Timed[];
  time: number;
  p: Palette;
  s: Strings;
  height: number;
}> = ({ toasts, time, p, s, height }) => {
  const specAt = (toast: Timed) => {
    let spec = toast.spec;
    for (const step of toast.replaces ?? []) if (time >= step.at) spec = step.spec;
    return spec;
  };
  // Bottom offset of a card at a given moment: heights of newer live cards below it.
  const live = (at: number) => toasts.filter((t) => t.at <= at && (t.leave === undefined || at < t.leave + EXIT));
  const offsetAt = (toast: Timed, at: number) => {
    const below = live(at).filter((t) => t.at > toast.at && (t.leave === undefined || at < t.leave));
    return below.reduce((sum, t) => sum + measure(specAt(t), p) + GAP, 0);
  };
  // Glide: interpolate the target offset over SETTLE after each change.
  const glided = (toast: Timed) => {
    const events = toasts
      .flatMap((t) => [t.at, ...(t.leave !== undefined ? [t.leave] : [])])
      .filter((e) => e > toast.at && e <= time)
      .sort((a, b) => a - b);
    let value = offsetAt(toast, toast.at);
    for (const e of events) {
      const target = offsetAt(toast, e + 0.0001);
      const k = easeOutCubic((time - e) / SETTLE);
      value = value + (target - value) * k;
    }
    return value;
  };
  return (
    <div style={{ position: "absolute", inset: 0 }}>
      {toasts
        .filter((t) => time >= t.at && (t.leave === undefined || time < t.leave + EXIT))
        .map((toast) => {
          const spec = specAt(toast);
          const age = time - toast.at;
          const enter = Math.min(1, age / ENTER);
          const exit = toast.leave !== undefined && time > toast.leave ? (time - toast.leave) / EXIT : 0;
          const slide = (1 - easeOutCubic(enter)) * CARD_WIDTH * 0.3 + easeOutCubic(exit) * CARD_WIDTH * 0.25;
          const alpha = Math.min(1, enter * 2) * (1 - easeOutCubic(exit));
          const h = measure(spec, p);
          const bottom = EDGE + glided(toast);
          return (
            <ToastCard
              key={toast.spec.id}
              t={spec}
              p={p}
              s={s}
              age={age}
              style={{ left: EDGE + slide, top: height - bottom - h, opacity: alpha }}
            />
          );
        })}
    </div>
  );
};
