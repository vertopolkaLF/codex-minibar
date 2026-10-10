import assert from "node:assert/strict";
import test from "node:test";

const { normalizeSettings, needsMigration, selectedProvider, renderIndicator } = await import(process.env.MINIBAR_RENDER_TEST_MODULE);
const window = used => ({ used_percent: used, remaining_percent: 100 - used, resets_at: null, duration_minutes: 300 });
const instance = (kind, id, used, badge = null) => ({
  id, kind, source_id: id, name: kind, badge, badge_rgb: badge ? [10, 200, 90] : null,
  account_name: "Same name", icon: kind, brand_rgb: [200, 100, 50],
  primary: window(used), secondary: window(used), additional: [],
  metrics: [{ id: `${kind}.session`, label: "5h", window: window(used) }],
});
const snapshot = {
  providers: [
    instance("claude", "claude-19a2b-1", 80, "WO"),
    instance("claude", "claude", 7, "CL"),
    instance("codex", "codex", 7),
    instance("codex", "work", 40),
  ],
};

test("keys select the primary instance by its driver id", () => {
  const settings = normalizeSettings({ provider: "claude" });
  assert.equal(selectedProvider(snapshot, settings).primary.remaining_percent, 93);
});

test("two keys for the same driver render their independently selected instances", () => {
  const settings = normalizeSettings({ provider: "claude", singleMetricId: "claude.session", presentation: "numbers", resetDisplay: "hidden", font: "segoe" });
  const work = { ...settings, provider: "claude-19a2b-1" };
  const firstSvg = decodeURIComponent(renderIndicator(snapshot, settings, true));
  const secondSvg = decodeURIComponent(renderIndicator(snapshot, work, true));
  assert.match(firstSvg, /93%/);
  assert.match(secondSvg, /20%/);
  assert.notEqual(firstSvg, secondSvg);
});

test("removed instances never fall back to another instance", () => {
  assert.equal(selectedProvider(snapshot, normalizeSettings({ provider: "claude-gone-1" })), null);
  const onlyWork = { providers: [snapshot.providers[0]] };
  assert.equal(selectedProvider(onlyWork, normalizeSettings({ provider: "claude" })), null);
});

test("legacy account selections migrate to the matching instance id", () => {
  const legacy = { provider: "codex", profileIds: { codex: "work", claude: "default" } };
  assert.equal(needsMigration(legacy), true);
  const settings = normalizeSettings(legacy);
  assert.equal(settings.provider, "work");
  assert.equal("profileIds" in settings, false);
  assert.equal(needsMigration(settings), false);
  assert.equal(selectedProvider(snapshot, settings).primary.remaining_percent, 60);
  assert.equal(normalizeSettings({ provider: "claude", profileIds: { claude: "default" } }).provider, "claude");
});

test("provider cycling migrates each account and keeps instances distinct", () => {
  const settings = normalizeSettings({
    clickAction: "cycle_provider",
    cycleProviders: ["claude", "codex", "cursor"],
    profileIds: { claude: "claude-19a2b-1", codex: "default" },
  });
  assert.deepEqual(settings.cycleProviders, ["claude-19a2b-1", "codex", "cursor"]);
  assert.equal(selectedProvider(snapshot, settings).primary.remaining_percent, 20);
  assert.equal(selectedProvider(snapshot, { ...settings, cycleIndex: 1 }).primary.remaining_percent, 93);
});

test("logo watermark carries the instance badge", () => {
  const settings = normalizeSettings({ provider: "claude-19a2b-1", singleMetricId: "claude.session", presentation: "rings", providerMark: "logo" });
  assert.match(decodeURIComponent(renderIndicator(snapshot, settings, true)), />WO</);
  const solo = normalizeSettings({ provider: "codex", presentation: "rings", providerMark: "logo" });
  assert.doesNotMatch(decodeURIComponent(renderIndicator(snapshot, solo, true)), /font-size="20" font-weight="800"/);
});
