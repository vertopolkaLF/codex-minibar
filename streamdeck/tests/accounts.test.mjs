import assert from "node:assert/strict";
import test from "node:test";

const { normalizeSettings, selectedProvider, renderIndicator } = await import(process.env.MINIBAR_RENDER_TEST_MODULE);
const window = used => ({ used_percent: used, remaining_percent: 100 - used, resets_at: null, duration_minutes: 300 });
const account = (provider, id, used) => ({
  id: provider, profile_id: id, source_id: `${provider}:profile:${id.length}:${id}`,
  name: provider, account_name: "Same name", icon: provider, brand_rgb: [200, 100, 50],
  primary: window(used), secondary: window(used), additional: [],
  metrics: [{ id: `${provider}.session`, label: "5h", window: window(used) }],
});
const snapshot = {
  providers: ["claude", "codex"].map(provider => ({
    ...account(provider, "default", 7),
    accounts: [account(provider, "work", 80), account(provider, "default", 7)],
  })),
};

test("legacy keys select Default by identity even when Work is first", () => {
  const settings = normalizeSettings({ provider: "claude" });
  assert.equal(selectedProvider(snapshot, settings).primary.remaining_percent, 93);
});

test("two keys for the same provider render their independently selected quotas", () => {
  const settings = normalizeSettings({ provider: "claude", singleMetricId: "claude.session", presentation: "numbers", resetDisplay: "hidden", font: "segoe" });
  const work = { ...settings, profileIds: { claude: "work" } };
  const firstSvg = decodeURIComponent(renderIndicator(snapshot, settings, true));
  const secondSvg = decodeURIComponent(renderIndicator(snapshot, work, true));
  assert.match(firstSvg, /93%/);
  assert.match(secondSvg, /20%/);
  assert.notEqual(firstSvg, secondSvg);
});

test("removed accounts and missing Default never fall back to another account", () => {
  const settings = normalizeSettings({ provider: "claude", profileIds: { claude: "removed" } });
  assert.equal(selectedProvider(snapshot, settings), null);
  const onlyWork = { providers: [{ ...snapshot.providers[0], accounts: [account("claude", "work", 80)] }] };
  assert.equal(selectedProvider(onlyWork, normalizeSettings({ provider: "claude" })), null);
  const oldBridge = { providers: [{ ...snapshot.providers[0], accounts: undefined }] };
  assert.equal(selectedProvider(oldBridge, settings), null);
  assert.equal(selectedProvider(oldBridge, normalizeSettings({ provider: "claude" })).primary.remaining_percent, 93);
});

test("provider cycling retains a separate selected account for each provider", () => {
  const settings = normalizeSettings({ clickAction: "cycle_provider", cycleProviders: ["claude", "codex"], profileIds: { claude: "work", codex: "default" } });
  assert.equal(selectedProvider(snapshot, settings).primary.remaining_percent, 20);
  assert.equal(selectedProvider(snapshot, { ...settings, cycleIndex: 1 }).primary.remaining_percent, 93);
});
