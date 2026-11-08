// Usage tab, after src/ui/popup_window/ui/usage.rs.
import React from "react";
import { Palette } from "./palette";
import { ProviderMark, Segmented, T, cardStyle } from "./kit";
import { COST_SERIES } from "../data";

const money = (v: number) => `$${v.toLocaleString("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 })}`;

const PROVIDERS = [
  { key: "claude", name: "Claude", color: "#FB9A44", sessions: 493, tokens: "2.0B", badge: undefined },
  { key: "codex", name: "Codex", color: "rgb(128,159,255)", sessions: 350, tokens: "3.8B", badge: undefined },
  { key: "cursor", name: "Cursor", color: "#ffffff", sessions: 1728, tokens: "719.2M", badge: undefined },
] as const;

/// Breakdown by model; per-provider sums match the totals above.
const MODELS = [
  { provider: "claude", model: "claude-opus-4-5", cost: 812.4, tokens: "1.1B" },
  { provider: "codex", model: "gpt-5.2-codex", cost: 402.15, tokens: "3.4B" },
  { provider: "claude", model: "claude-sonnet-4-5", cost: 335.83, tokens: "874.2M" },
  { provider: "cursor", model: "composer-1", cost: 118.3, tokens: "512.4M" },
  { provider: "cursor", model: "auto", cost: 38.57, tokens: "206.8M" },
  { provider: "claude", model: "claude-haiku-4-5", cost: 37.57, tokens: "54.3M" },
  { provider: "codex", model: "gpt-5.1-codex-mini", cost: 32.57, tokens: "410.0M" },
];

const Breakdown: React.FC<{ p: Palette; total: number }> = ({ p, total }) => {
  const cell = (width: number, text: string, color: string) => (
    <T size={12} color={color} style={{ width, textAlign: "right", flex: "none" }}>
      {text}
    </T>
  );
  return (
    <div style={{ ...cardStyle(p), padding: 14, display: "flex", flexDirection: "column", gap: 14 }}>
      <div style={{ display: "flex", alignItems: "center" }}>
        <T size={14} color={p.textPrimary} weight={600}>
          Breakdown
        </T>
        <div style={{ flex: 1 }} />
        <Segmented items={["Model", "Day"]} selected={0} p={p} />
      </div>
      <div style={{ display: "flex", flexDirection: "column", gap: 6 }}>
        <div style={{ display: "flex", alignItems: "center" }}>
          <T size={12} color={p.textTertiary} style={{ flex: 1 }}>
            Model
          </T>
          {cell(56, "Cost", p.textTertiary)}
          {cell(44, "Share", p.textTertiary)}
          {cell(56, "Tokens", p.textTertiary)}
        </div>
        {MODELS.map((row) => (
          <div key={row.model} style={{ display: "flex", alignItems: "center" }}>
            <div style={{ flex: 1, minWidth: 0, display: "flex", alignItems: "center", gap: 6 }}>
              <ProviderMark provider={row.provider} size={14} p={p} />
              <T size={12} color={p.textPrimary} style={{ whiteSpace: "nowrap" }}>
                {row.model}
              </T>
            </div>
            {cell(56, money(row.cost), p.textPrimary)}
            {cell(44, `${((row.cost / total) * 100).toFixed(1)}%`, p.textPrimary)}
            {cell(56, row.tokens, p.textPrimary)}
          </div>
        ))}
      </div>
    </div>
  );
};

export const UsagePage: React.FC<{ p: Palette; draw?: number; period?: number; scroll?: number }> = ({
  p,
  draw = 1,
  period = 2,
  scroll = 0,
}) => {
  const totals = PROVIDERS.map((pr) =>
    COST_SERIES.reduce((sum, day) => sum + day[pr.key as "claude" | "codex" | "cursor"], 0),
  );
  const total = totals.reduce((a, b) => a + b, 0);
  const shown = total * Math.min(1, draw * 1.4);
  const max = Math.max(...COST_SERIES.map((d) => Math.max(d.claude, d.codex, d.cursor)));
  const W = 268;
  const H = 132;
  const n = COST_SERIES.length;
  const visible = Math.max(2, Math.floor(n * draw));
  const path = (key: "claude" | "codex" | "cursor") =>
    COST_SERIES.slice(0, visible)
      .map((d, i) => `${i ? "L" : "M"}${(i / (n - 1)) * W},${H - (d[key] / max) * (H - 8)}`)
      .join(" ");
  return (
    <div
      style={{
        padding: "4px 16px 16px",
        display: "flex",
        flexDirection: "column",
        gap: 6,
        transform: `translateY(${-scroll}px)`,
      }}
    >
      <div style={{ display: "flex", alignItems: "center", height: 44, padding: "0 4px" }}>
        <div>
          <T size={14} color={p.textPrimary} weight={600}>
            Usage
          </T>
          <T size={12} color={p.textTertiary}>
            Sep 12 to Oct 11
          </T>
        </div>
        <div style={{ flex: 1 }} />
        <Segmented items={["24h", "7d", "30d", "90d"]} selected={Math.round(period)} glide={period} p={p} />
      </div>
      <div style={{ ...cardStyle(p), padding: 14 }}>
        <div style={{ display: "flex", alignItems: "flex-start" }}>
          <div style={{ flex: 1 }}>
            <T size={28} line={36} color={p.textPrimary} weight={600} style={{ fontVariantNumeric: "tabular-nums" }}>
              {money(shown)}
            </T>
            <T size={12} color={p.textTertiary}>
              2571 sessions · API estimate
            </T>
          </div>
          <Segmented items={["Cost", "Tokens"]} selected={0} p={p} />
        </div>
        <div style={{ display: "flex", gap: 3, height: 6, margin: "12px 0" }}>
          {PROVIDERS.map((pr, i) => (
            <div key={pr.key} style={{ flex: totals[i], borderRadius: 3, background: pr.color }} />
          ))}
        </div>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", rowGap: 10, columnGap: 12 }}>
          {PROVIDERS.map((pr, i) => (
            <div key={pr.key}>
              <div style={{ display: "flex", alignItems: "center", gap: 6 }}>
                <ProviderMark provider={pr.key} size={14} p={p} />
                <T size={14} color={p.textPrimary} weight={600}>
                  {pr.name}
                </T>
              </div>
              <T size={12} color={p.textSecondary}>
                {money(totals[i] * Math.min(1, draw * 1.4))} · {pr.sessions} sessions
              </T>
              <T size={12} color={p.textTertiary}>
                {((totals[i] / total) * 100).toFixed(1)}% of cost · {pr.tokens}
              </T>
            </div>
          ))}
        </div>
      </div>
      <div style={{ ...cardStyle(p), padding: 14 }}>
        <T size={14} color={p.textPrimary} weight={600} style={{ marginBottom: 8 }}>
          Cost
        </T>
        <div style={{ display: "flex", gap: 6 }}>
          <div
            style={{
              width: 40,
              height: H,
              display: "flex",
              flexDirection: "column",
              justifyContent: "space-between",
              alignItems: "flex-end",
            }}
          >
            {[max, (max * 2) / 3, max / 3, 0].map((v) => (
              <T key={v} size={11} line={12} color={p.textTertiary}>
                ${v.toFixed(2)}
              </T>
            ))}
          </div>
          <svg width={W} height={H} style={{ overflow: "visible" }}>
            {[0, 1, 2, 3].map((i) => (
              <line key={i} x1={0} x2={W} y1={(H * i) / 3} y2={(H * i) / 3} stroke={p.chartGrid} strokeDasharray="3 3" />
            ))}
            {(["cursor", "codex", "claude"] as const).map((key) => {
              const pr = PROVIDERS.find((x) => x.key === key)!;
              return (
                <g key={key}>
                  <path
                    d={`${path(key)} L${((visible - 1) / (n - 1)) * W},${H} L0,${H} Z`}
                    fill={pr.color}
                    opacity={0.12}
                  />
                  <path d={path(key)} fill="none" stroke={pr.color} strokeWidth={2} strokeLinejoin="round" />
                </g>
              );
            })}
          </svg>
        </div>
        <div style={{ display: "flex", justifyContent: "space-between", paddingLeft: 46, marginTop: 4 }}>
          {["Sep 12", "Sep 27", "Oct 11"].map((d) => (
            <T key={d} size={11} line={12} color={p.textTertiary}>
              {d}
            </T>
          ))}
        </div>
      </div>
      <Breakdown p={p} total={total} />
      <div style={{ ...cardStyle(p), padding: 14 }}>
        <T size={14} color={p.textPrimary} weight={600} style={{ marginBottom: 8 }}>
          Totals
        </T>
        <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", rowGap: 8 }}>
          {[
            ["Processed tokens", "6.5B"],
            ["Cached input", "6.3B"],
            ["Uncached input", "198.4M"],
            ["Output", "27.1M"],
          ].map(([label, value]) => (
            <div key={label}>
              <T size={12} color={p.textTertiary}>
                {label}
              </T>
              <T size={14} color={p.textPrimary} weight={600}>
                {value}
              </T>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
};
