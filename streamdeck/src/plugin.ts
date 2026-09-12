import { appendFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";

import streamDeck from "@elgato/streamdeck";

import { QuotaIndicator } from "./action";

function diagnostic(message: string): void {
  try {
    appendFileSync(join(tmpdir(), "codex-minibar-streamdeck.log"), `${new Date().toISOString()} ${message}\n`);
  } catch {
    // Diagnostics must never prevent the action from starting.
  }
}

diagnostic("plugin starting");
try {
  streamDeck.actions.registerAction(new QuotaIndicator());
  diagnostic("action registered");
  void streamDeck.connect().catch(error => diagnostic(`connection failed: ${String(error)}`));
} catch (error) {
  diagnostic(`startup failed: ${String(error)}`);
}
