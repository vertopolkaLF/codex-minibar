import { copyFileSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const plugin = path.join(root, "com.vertopolkalf.codex-minibar.sdPlugin");
const bin = path.join(plugin, "bin");

mkdirSync(bin, { recursive: true });
copyFileSync(path.join(plugin, "manifest.json"), path.join(bin, "manifest.json"));
