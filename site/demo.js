/* Interactive mockup of the real Codex Minibar popup.
   Layout, wording and metrics mirror src/popup_window/* so the site preview
   stays honest about what the app actually shows. */
(() => {
  const mock = document.querySelector(".popup-mock");
  if (!mock) return;

  const content = mock.querySelector("[data-demo-content]");
  const footer = mock.querySelector("[data-demo-footer]");

  /* ---------------------------------------------------------------- data */

  const PROVIDERS = {
    codex: {
      name: "Codex",
      plan: "Plus",
      account: "vertopolkaLF",
      icon: "assets/provider-chatgpt.svg",
      cls: "icon-codex",
      color: "#809fff",
    },
    claude: {
      name: "Claude",
      plan: "Team",
      account: "Pavel Leontiev",
      icon: "assets/provider-claude.svg",
      cls: "icon-claude",
      color: "#d97757",
    },
    cursor: {
      name: "Cursor",
      plan: "Pro",
      account: null,
      icon: "assets/provider-cursor.svg",
      cls: "icon-cursor",
      color: "#e6e6e6",
    },
    openrouter: {
      name: "OpenRouter",
      plan: null,
      account: null,
      icon: "assets/provider-openrouter.svg",
      cls: "icon-openrouter",
      color: "#c8ff00",
    },
  };

  const LIMITS = {
    codex: [
      { title: "5h session", left: "0% left", fill: 0, reset: "3h 57m", ticks: 4 },
      {
        title: "Weekly",
        left: "3% left",
        fill: 3,
        reset: "4d 10h",
        ticks: 6,
        pace: { label: "61% in deficit", at: 61 },
      },
    ],
    claude: [
      { title: "5h session", left: "99% left", fill: 99, reset: "4h 56m", ticks: 4 },
      {
        title: "Weekly",
        left: "82% left",
        fill: 82,
        reset: "4d 13h",
        ticks: 6,
        pace: { label: "17% in reserve", at: 65 },
      },
    ],
    cursor: [
      { title: "Cursor models", left: "0% left", fill: 0, reset: "3d 6h", ticks: 3 },
      { title: "Other models", left: "0% left", fill: 0, reset: "3d 6h", ticks: 3 },
    ],
  };

  const BANKED = {
    codex: { amount: "2 Banked Resets", expires: "Expires in 27d", at: "Oct 4, 04:14" },
  };

  const SPEND = {
    today: {
      total: "$18.68",
      parts: [["claude", 1380, "$13.80"], ["codex", 488, "$4.88"], ["cursor", 0, "$0.00"]],
    },
    yesterday: {
      total: "$24.13",
      parts: [["claude", 1520, "$15.20"], ["codex", 761, "$7.61"], ["cursor", 132, "$1.32"]],
    },
    "30 days": {
      total: "$1,696.66",
      parts: [["claude", 110900, "$1.1K"], ["codex", 29800, "$298"], ["cursor", 28900, "$289"]],
    },
  };

  const PROVIDER_STATS = {
    codex: {
      today: ["128.4K", "≈ $4.88"],
      month: ["3.1B", "≈ $298"],
      footer: "2.9B in · 4.1M out · 2.8B cached · 12403 requests",
      seed: 7,
    },
    claude: {
      today: ["592.5K", "≈ $13.80"],
      month: ["43.3M", "≈ $1.1K"],
      footer: "36.8M in · 6.5M out · 65.2K cached · 8690 requests",
      seed: 3,
    },
    cursor: {
      today: ["0", "≈ $0.00"],
      month: ["576.1M", "≈ $289"],
      footer: "561.2M in · 3.4M out · 512.0M cached · 21870 requests",
      seed: 11,
    },
  };

  const RANGES = {
    "Past 24h": {
      dates: "Sep 6 to Sep 7",
      total: "$18.68",
      sessions: "23",
      axis: ["$4.10", "$2.73", "$1.37", "$0"],
      points: 24,
      legend: [
        ["claude", "$13.80", "9 sessions", "73.9% of cost", "592.5K"],
        ["codex", "$4.88", "11 sessions", "26.1% of cost", "128.4K"],
        ["cursor", "$0.00", "3 sessions", "0.0% of cost", "0"],
      ],
      totals: [
        ["Processed tokens", "721.0K"],
        ["Cached input", "65.2K"],
        ["Uncached input", "608.9K"],
        ["Output", "46.8K"],
        ["Cache savings", "$21.40"],
      ],
      models: [
        ["claude", "claude-opus-5", "$9.12", "48.8%", "312.6K"],
        ["claude", "claude-sonnet-5", "$4.68", "25.1%", "279.9K"],
        ["codex", "gpt-5.2-codex", "$3.21", "17.2%", "96.4K"],
        ["codex", "gpt-5.1-codex", "$1.67", "8.9%", "32.0K"],
      ],
      days: [
        ["Sep 7", "$6.42", "34.4%", "241.1K"],
        ["Sep 6", "$12.26", "65.6%", "479.9K"],
      ],
    },
    "7 days": {
      dates: "Sep 1 to Sep 7",
      total: "$284.31",
      sessions: "612",
      axis: ["$96", "$64", "$32", "$0"],
      points: 7,
      legend: [
        ["claude", "$186", "108 sessions", "65.4% of cost", "9.7M"],
        ["codex", "$51.9", "142 sessions", "18.3% of cost", "701.4M"],
        ["cursor", "$46.4", "362 sessions", "16.3% of cost", "128.8M"],
      ],
      totals: [
        ["Processed tokens", "840.5M"],
        ["Cached input", "801.2M"],
        ["Uncached input", "34.8M"],
        ["Output", "4.5M"],
        ["Cache savings", "$1.9K"],
      ],
      models: [
        ["claude", "claude-opus-5", "$142", "50.0%", "6.4M"],
        ["claude", "claude-opus-4.8", "$44.1", "15.5%", "1.7M"],
        ["codex", "gpt-5.2-codex", "$28.8", "10.1%", "430.2M"],
        ["cursor", "composer-1", "$24.7", "8.7%", "89.1M"],
        ["claude", "claude-sonnet-5", "$21.3", "7.5%", "1.6M"],
        ["codex", "gpt-5.1-codex", "$17.1", "6.0%", "271.2M"],
      ],
      days: [
        ["Sep 7", "$18.68", "6.6%", "721.0K"],
        ["Sep 6", "$52.41", "18.4%", "128.4M"],
        ["Sep 5", "$47.90", "16.8%", "141.9M"],
        ["Sep 4", "$61.03", "21.5%", "203.7M"],
        ["Sep 3", "$38.12", "13.4%", "112.5M"],
        ["Sep 2", "$44.09", "15.5%", "186.2M"],
        ["Sep 1", "$22.08", "7.8%", "67.1M"],
      ],
    },
    "30 days": {
      dates: "Aug 9 to Sep 7",
      total: "$1,696.66",
      sessions: "2867",
      axis: ["$111", "$74", "$37", "$0"],
      points: 30,
      legend: [
        ["claude", "$1.1K", "487 sessions", "65.4% of cost", "43.3M"],
        ["codex", "$298", "605 sessions", "17.6% of cost", "3.1B"],
        ["cursor", "$289", "1775 sessions", "17.0% of cost", "576.1M"],
      ],
      totals: [
        ["Processed tokens", "3.8B"],
        ["Cached input", "3.6B"],
        ["Uncached input", "160.6M"],
        ["Output", "19.9M"],
        ["Cache savings", "$8.6K"],
      ],
      models: [
        ["claude", "claude-opus-5", "$823", "48.5%", "28.6M"],
        ["claude", "claude-opus-4.8", "$238", "14.0%", "7.6M"],
        ["codex", "gpt-5.2-codex", "$166", "9.8%", "1.9B"],
        ["cursor", "composer-1", "$154", "9.1%", "402.1M"],
        ["claude", "claude-sonnet-5", "$131", "7.7%", "6.4M"],
        ["codex", "gpt-5.1-codex", "$98.4", "5.8%", "1.1B"],
        ["cursor", "cursor-fast", "$85.6", "5.1%", "174.0M"],
      ],
      days: [
        ["Sep 7", "$18.68", "1.1%", "721.0K"],
        ["Sep 6", "$52.41", "3.1%", "128.4M"],
        ["Sep 5", "$47.90", "2.8%", "141.9M"],
        ["Sep 4", "$61.03", "3.6%", "203.7M"],
        ["Sep 3", "$38.12", "2.2%", "112.5M"],
        ["Sep 2", "$44.09", "2.6%", "186.2M"],
        ["Sep 1", "$22.08", "1.3%", "67.1M"],
      ],
    },
    "90 days": {
      dates: "Jun 9 to Sep 7",
      total: "$4,912.40",
      sessions: "8104",
      axis: ["$128", "$85", "$43", "$0"],
      points: 45,
      legend: [
        ["claude", "$3.1K", "1382 sessions", "63.1% of cost", "121.7M"],
        ["codex", "$941", "1806 sessions", "19.2% of cost", "9.2B"],
        ["cursor", "$868", "4916 sessions", "17.7% of cost", "1.6B"],
      ],
      totals: [
        ["Processed tokens", "11.0B"],
        ["Cached input", "10.4B"],
        ["Uncached input", "487.3M"],
        ["Output", "58.1M"],
        ["Cache savings", "$24.9K"],
      ],
      models: [
        ["claude", "claude-opus-5", "$2.2K", "44.8%", "78.1M"],
        ["claude", "claude-opus-4.8", "$684", "13.9%", "24.3M"],
        ["codex", "gpt-5.2-codex", "$512", "10.4%", "5.4B"],
        ["cursor", "composer-1", "$451", "9.2%", "1.1B"],
        ["claude", "claude-sonnet-5", "$402", "8.2%", "19.3M"],
        ["codex", "gpt-5.1-codex", "$328", "6.7%", "3.8B"],
        ["cursor", "cursor-fast", "$296", "6.0%", "512.4M"],
      ],
      days: [
        ["Sep 7", "$18.68", "0.4%", "721.0K"],
        ["Sep 6", "$52.41", "1.1%", "128.4M"],
        ["Sep 5", "$47.90", "1.0%", "141.9M"],
        ["Sep 4", "$61.03", "1.2%", "203.7M"],
        ["Sep 3", "$38.12", "0.8%", "112.5M"],
        ["Sep 2", "$44.09", "0.9%", "186.2M"],
        ["Sep 1", "$22.08", "0.4%", "67.1M"],
      ],
    },
  };

  const OPENROUTER = [
    {
      name: "Personal",
      spend: "$3.10",
      keys: [
        {
          label: "MAIN",
          masked: "sk-or-v1-3f…a9d",
          usage: "$3.10 / $100.0K",
          fill: 4,
          expires: "357d 2h",
        },
      ],
    },
    {
      name: "Sandbox",
      spend: "$34.88",
      keys: [
        { label: "CI RUNNER", masked: "sk-or-v1-a4…1f8", usage: "$21.64 / $50.00", fill: 43 },
        { label: "SCRATCH", masked: "sk-or-v1-77…c02", usage: "$13.24", fill: 0 },
      ],
    },
    {
      name: "Prod relay",
      spend: "$212.05",
      keys: [
        { label: "EDGE EU", masked: "sk-or-v1-b1…4de", usage: "$96.40 / $150.00", fill: 64 },
        {
          label: "EDGE US",
          masked: "sk-or-v1-9c…70a",
          usage: "$88.15 / $150.00",
          fill: 59,
          expires: "89d 14h",
        },
        { label: "FALLBACK", masked: "sk-or-v1-2e…bb6", usage: "$27.50", fill: 0 },
      ],
    },
  ];

  const TABS = [
    { id: "home", tip: "Home", icon: "assets/fluent-home.svg", cls: "icon-home" },
    { id: "usage", tip: "Usage", icon: "assets/fluent-usage.svg", cls: "icon-usage" },
    { id: "codex", tip: "Codex" },
    { id: "claude", tip: "Claude" },
    { id: "cursor", tip: "Cursor" },
    { id: "openrouter", tip: "OpenRouter" },
  ];

  /* --------------------------------------------------------------- state */

  const state = {
    view: "home",
    period: "today",
    range: "30 days",
    metric: "Cost",
    breakdown: "Model",
    refreshing: false,
  };

  /* ------------------------------------------------------------- helpers */

  const ENTITIES = { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" };
  const esc = (value) => String(value).replace(/[&<>"]/g, (ch) => ENTITIES[ch]);

  const icon = (src, cls) => `<img class="demo-icon ${cls}" src="${src}" alt="" />`;
  const providerIcon = (key) => icon(PROVIDERS[key].icon, PROVIDERS[key].cls);

  const GRIP =
    '<svg class="demo-icon" viewBox="0 0 16 16" width="16" height="16" aria-hidden="true">' +
    '<g fill="currentColor"><circle cx="6" cy="3.5" r="1.35"/><circle cx="10" cy="3.5" r="1.35"/>' +
    '<circle cx="6" cy="8" r="1.35"/><circle cx="10" cy="8" r="1.35"/>' +
    '<circle cx="6" cy="12.5" r="1.35"/><circle cx="10" cy="12.5" r="1.35"/></g></svg>';

  const handle = () =>
    `<button class="demo-handle" type="button" title="Drag to reorder"
      aria-label="Reorder">${GRIP}</button>`;

  /* Deterministic pseudo-random series so the mock never reshuffles. */
  const series = (seed, count, floor, volatility) => {
    let value = seed * 9301 + 49297;
    const next = () => {
      value = (value * 9301 + 49297) % 233280;
      return value / 233280;
    };
    const out = [];
    let level = 0.45;
    for (let i = 0; i < count; i += 1) {
      level = Math.min(1, Math.max(floor, level + (next() - 0.47) * volatility));
      out.push(level);
    }
    return out;
  };

  /* Daily cost curves are smooth in the app — sample noise is averaged out. */
  const smooth = (values) =>
    values.map((value, i) => {
      const prev = values[Math.max(0, i - 1)];
      const next = values[Math.min(values.length - 1, i + 1)];
      return (prev + value * 2 + next) / 4;
    });

  const areaPath = (values, w, h) => {
    const step = w / (values.length - 1);
    const points = values.map((v, i) => [i * step, h - v * h * 0.92]);
    const line = points
      .map(([x, y], i) => `${i ? "L" : "M"}${x.toFixed(1)} ${y.toFixed(1)}`)
      .join(" ");
    return { line, area: `${line} L${w} ${h} L0 ${h} Z` };
  };

  /* ---------------------------------------------------------- components */

  const limitCard = (limit) => {
    const seg = (100 / (limit.ticks + 1)).toFixed(4);
    const track = `<span class="demo-limit-track" style="--seg:${seg}%"></span>`;
    const marker = limit.pace
      ? `<span class="demo-marker" style="--marker:${limit.pace.at}%"></span>`
      : "";
    const reset = limit.reset
      ? `<span class="demo-reset"><em>Resets in</em> ${esc(limit.reset)}</span>`
      : '<span class="demo-reset"><em>Session not started</em></span>';

    if (!limit.pace) {
      return `<div class="demo-limit single" style="--fill:${limit.fill}%">
        <div class="demo-row">
          <span class="demo-cap">${esc(limit.title.toUpperCase())}</span>
          <strong>${esc(limit.left)}</strong>
          ${reset}
        </div>
        ${track}
      </div>`;
    }

    return `<div class="demo-limit" style="--fill:${limit.fill}%">
      ${marker}
      <div class="demo-row">
        <span>${esc(limit.title.toUpperCase())}</span>
        <span>${esc(limit.pace.label)}</span>
      </div>
      <div class="demo-row">
        <strong>${esc(limit.left)}</strong>
        ${reset}
      </div>
      ${track}
    </div>`;
  };

  const bankedCard = (banked) => `<div class="demo-surface demo-banked">
    <strong>${esc(banked.amount)}</strong>
    <span><b>${esc(banked.expires)}</b><small>${esc(banked.at)}</small></span>
  </div>`;

  const providerHead = (key, linked, reorderable) => {
    const provider = PROVIDERS[key];
    const title = `${esc(provider.name)}${provider.plan ? ` <em>${esc(provider.plan)}</em>` : ""}`;
    const heading = linked
      ? `<b><button type="button" data-view="${key}" title="Open ${esc(provider.name)}">${title}</button></b>`
      : `<b>${title}</b>`;
    return `<div class="demo-section-head">
      ${heading}
      <span class="demo-spacer"></span>
      ${provider.account ? `<span class="demo-account-name">${esc(provider.account)}</span>` : ""}
      ${reorderable ? handle() : ""}
    </div>`;
  };

  const providerSection = (key) => `<section>
    ${providerHead(key, true, true)}
    ${LIMITS[key].map(limitCard).join("")}
    ${BANKED[key] ? bankedCard(BANKED[key]) : ""}
  </section>`;

  const stackedBar = (parts) => {
    const total = parts.reduce((sum, part) => sum + part[1], 0) || 1;
    const segments = parts
      .map((part) => {
        const share = Math.max(0.45, (part[1] / total) * 100);
        return `<i style="flex:${share.toFixed(3)};background:${PROVIDERS[part[0]].color}"></i>`;
      })
      .join("");
    return `<div class="demo-stacked">${segments}</div>`;
  };

  const usageStatsSection = () => {
    const spend = SPEND[state.period];
    const periods = ["today", "yesterday", "30 days"]
      .map((period) => {
        const label = period === "30 days" ? "30 days" : period[0].toUpperCase() + period.slice(1);
        return `<button type="button" data-period="${period}" aria-pressed="${
          state.period === period
        }">${label}</button>`;
      })
      .join("");
    const legend = spend.parts
      .map(
        (part) => `<span>
          ${providerIcon(part[0])}<b>${esc(PROVIDERS[part[0]].name)}</b>
          <small>${esc(part[2])}</small>
        </span>`
      )
      .join("");

    return `<section>
      <div class="demo-section-head">
        <b><button type="button" data-view="usage" title="Open Usage">Usage Stats</button></b>
        <span class="demo-spacer"></span>
        <span class="demo-periods">${periods}</span>
        ${handle()}
      </div>
      <button class="demo-surface demo-stats-card" type="button" data-view="usage" title="Open Usage">
        <span class="demo-total">${esc(spend.total)}</span>
        ${stackedBar(spend.parts)}
        <span class="demo-legend mini">${legend}</span>
      </button>
    </section>`;
  };

  /* --------------------------------------------------------------- views */

  const homeView = () =>
    usageStatsSection() + ["codex", "claude", "cursor"].map(providerSection).join("");

  const providerView = (key) => {
    const stats = PROVIDER_STATS[key];
    const bars = series(stats.seed, 30, 0.08, 0.85)
      .map((v) => `<div style="height:${(v * 100).toFixed(1)}%"></div>`)
      .join("");
    return `<section>
      ${providerHead(key, false, false)}
      ${LIMITS[key].map(limitCard).join("")}
      ${BANKED[key] ? bankedCard(BANKED[key]) : ""}
      <div class="demo-surface demo-spark">
        <div class="demo-row">
          <span>
            <small>Today</small>
            <b>${esc(stats.today[0])}</b> <em>${esc(stats.today[1])}</em>
          </span>
          <span class="demo-right">
            <small>Last 30 days</small>
            <b>${esc(stats.month[0])}</b> <em>${esc(stats.month[1])}</em>
          </span>
        </div>
        <div class="demo-bars">${bars}</div>
        <small>${esc(stats.footer)}</small>
      </div>
    </section>`;
  };

  const openrouterView = () => {
    const accounts = OPENROUTER.map((account) => {
      const keys = account.keys
        .map(
          (key) => `<div class="demo-limit demo-key" style="--fill:${key.fill}%">
            <span>
              <b>${esc(key.label)}</b>
              <small>${esc(key.masked)}</small>
            </span>
            <span class="demo-right">
              <small><em>Usage:</em> <strong>${esc(key.usage)}</strong></small>
              ${
                key.expires
                  ? `<small><em>Expires in</em> <span>${esc(key.expires)}</span></small>`
                  : ""
              }
            </span>
          </div>`
        )
        .join("");
      return `<div class="demo-account">
        <div class="demo-row"><b>${esc(account.name)}</b><strong>${esc(account.spend)}</strong></div>
        ${keys}
      </div>`;
    }).join("");

    return `<section>
      <div class="demo-section-head"><b>OpenRouter</b></div>
      ${accounts}
    </section>`;
  };

  const usageView = () => {
    const range = RANGES[state.range];
    const paths = ["claude", "codex", "cursor"]
      .map((key, index) => {
        const shape = areaPath(
          smooth(smooth(series(range.points + index * 5, range.points, 0.08, 0.45))),
          300,
          150
        );
        const color = PROVIDERS[key].color;
        const opacity = key === "cursor" ? 0.07 : 0.22;
        return `<path d="${shape.area}" fill="${color}" fill-opacity="${opacity}"></path>
          <path d="${shape.line}" fill="none" stroke="${color}" stroke-width="1.6"
            vector-effect="non-scaling-stroke" stroke-linejoin="round"></path>`;
      })
      .join("");

    const legend = range.legend
      .map(
        (entry) => `<span>
          ${providerIcon(entry[0])}<b>${esc(PROVIDERS[entry[0]].name)}</b>
          <span><strong>${esc(entry[1])}</strong> <em>&middot; ${esc(entry[2])}</em></span>
          <small>${esc(entry[3])} &middot; ${esc(entry[4])}</small>
        </span>`
      )
      .join("");

    const isModel = state.breakdown === "Model";
    const rows = (isModel ? range.models : range.days)
      .map((row) => {
        const label = isModel ? row[1] : row[0];
        const cells = (isModel ? row.slice(2) : row.slice(1))
          .map((cell) => `<td>${esc(cell)}</td>`)
          .join("");
        const badge = isModel ? providerIcon(row[0]) : "";
        return `<tr><td>${badge}${esc(label)}</td>${cells}</tr>`;
      })
      .join("");

    const [from, to] = range.dates.split(" to ");

    return `<section>
      <div class="demo-section-head demo-usage-head">
        <b>Usage</b> <em>${esc(range.dates)}</em>
        <span class="demo-spacer"></span>
        <span class="demo-segments demo-inline-segments">
          ${["Cost", "Tokens"]
            .map(
              (metric) =>
                `<button type="button" data-metric="${metric}" aria-pressed="${
                  state.metric === metric
                }">${metric}</button>`
            )
            .join("")}
        </span>
      </div>

      <div class="demo-segments">
        ${Object.keys(RANGES)
          .map(
            (key) =>
              `<button type="button" data-range="${esc(key)}" aria-pressed="${
                state.range === key
              }">${esc(key)}</button>`
          )
          .join("")}
      </div>

      <div class="demo-surface">
        <div class="demo-row">
          <span class="demo-total">${esc(range.total)}</span>
          <span class="demo-right">
            <b>${esc(range.sessions)} sessions</b>
            <small>API estimate</small>
          </span>
        </div>
        ${stackedBar(range.legend.map((entry) => [entry[0], parseFloat(entry[3]) * 10]))}
        <div class="demo-legend">${legend}</div>
      </div>

      <div class="demo-surface">
        <b>${esc(state.metric)}</b>
        <div class="demo-chart">
          <div class="demo-axis">${range.axis
            .map((label) => `<span>${esc(label)}</span>`)
            .join("")}</div>
          <svg viewBox="0 0 300 150" preserveAspectRatio="none" aria-hidden="true">
            <line x1="0" y1="1" x2="300" y2="1" stroke="currentColor" stroke-opacity=".22"
              stroke-width="1" vector-effect="non-scaling-stroke"></line>
            ${paths}
          </svg>
        </div>
        <div class="demo-row demo-chart-dates">
          <small>${esc(from)}</small>
          <small>${esc(to)}</small>
        </div>
      </div>

      <div class="demo-surface">
        <b>Totals</b>
        <dl class="demo-totals">
          ${range.totals
            .map((total) => `<div><dt>${esc(total[0])}</dt><dd>${esc(total[1])}</dd></div>`)
            .join("")}
        </dl>
      </div>

      <div class="demo-surface">
        <div class="demo-row">
          <b>Breakdown</b>
          <span class="demo-segments demo-inline-segments">
            ${["Model", "Day"]
              .map(
                (mode) =>
                  `<button type="button" data-breakdown="${mode}" aria-pressed="${
                    state.breakdown === mode
                  }">${mode}</button>`
              )
              .join("")}
          </span>
        </div>
        <table class="demo-table">
          <thead><tr><th>${state.breakdown}</th><th>Cost</th><th>Share</th><th>Tokens</th></tr></thead>
          <tbody>${rows}</tbody>
        </table>
      </div>
    </section>`;
  };

  const VIEWS = {
    home: homeView,
    usage: usageView,
    codex: () => providerView("codex"),
    claude: () => providerView("claude"),
    cursor: () => providerView("cursor"),
    openrouter: openrouterView,
  };

  /* -------------------------------------------------------------- render */

  const renderFooter = () => {
    const tabs = TABS.map((tab) => {
      const provider = PROVIDERS[tab.id];
      const glyph = provider ? providerIcon(tab.id) : icon(tab.icon, tab.cls);
      return `<button class="demo-tab" type="button" data-view="${tab.id}" title="${esc(tab.tip)}"
        aria-label="${esc(tab.tip)}" aria-pressed="${state.view === tab.id}">${glyph}</button>`;
    }).join("");

    footer.innerHTML = `<div class="demo-tabs">${tabs}</div>
      <div class="demo-actions">
        <button type="button" data-action="refresh" aria-label="Refresh"
          title="Refresh | Last updated just now"
          class="${state.refreshing ? "is-refreshing" : ""}">${icon(
      "assets/fluent-refresh-filled.svg",
      "icon-refresh-filled"
    )}</button>
        <button type="button" data-action="settings" title="Settings" aria-label="Settings">${icon(
          "assets/fluent-settings-filled.svg",
          "icon-settings-filled"
        )}</button>
      </div>`;
  };

  const render = () => {
    content.innerHTML = VIEWS[state.view]();
    renderFooter();
    content.scrollTop = 0;
    window.dispatchEvent(new CustomEvent("demo:rendered"));
  };

  /* -------------------------------------------------------------- events */

  mock.addEventListener("click", (event) => {
    const target = event.target.closest("button");
    if (!target || !mock.contains(target)) return;

    const data = target.dataset;
    if (data.view) {
      state.view = data.view;
      render();
      return;
    }
    if (data.period) {
      state.period = data.period;
      render();
      return;
    }
    if (data.range) {
      state.range = data.range;
      render();
      return;
    }
    if (data.metric) {
      state.metric = data.metric;
      render();
      return;
    }
    if (data.breakdown) {
      state.breakdown = data.breakdown;
      render();
      return;
    }
    if (data.action === "refresh") {
      if (state.refreshing) return;
      state.refreshing = true;
      renderFooter();
      window.setTimeout(() => {
        state.refreshing = false;
        renderFooter();
      }, 950);
      return;
    }
  });

  render();
})();
