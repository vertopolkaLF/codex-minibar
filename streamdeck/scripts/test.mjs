import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { pathToFileURL } from "node:url";
import { rollup } from "rollup";
import typescript from "@rollup/plugin-typescript";

const directory = await mkdtemp(path.join(tmpdir(), "minibar-streamdeck-tests-"));
try {
  const bundle = await rollup({
    input: "src/render.ts",
    external: id => id.startsWith("node:"),
    plugins: [typescript({ tsconfig: "./tsconfig.json" })],
  });
  const output = path.join(directory, "render.mjs");
  await bundle.write({ file: output, format: "es" });
  await bundle.close();
  const result = spawnSync(process.execPath, ["--test", "tests/accounts.test.mjs"], {
    stdio: "inherit",
    env: { ...process.env, MINIBAR_RENDER_TEST_MODULE: pathToFileURL(output).href },
  });
  process.exitCode = result.status ?? 1;
} finally {
  if (path.dirname(path.resolve(directory)) !== path.resolve(tmpdir()) || !path.basename(directory).startsWith("minibar-streamdeck-tests-")) {
    throw new Error("Unexpected test output directory");
  }
  await rm(directory, { recursive: true, force: true });
}
