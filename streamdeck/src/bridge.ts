import { existsSync, readFile } from "node:fs";
import { spawn } from "node:child_process";
import path from "node:path";
import net from "node:net";

export interface BridgeEndpoint {
  protocol: number;
  host: string;
  port: number;
  token: string;
  executable: string;
}

export interface MetricInfo {
  id: string;
  label: string;
}

export interface ProviderInfo {
  id: string;
  name: string;
  icon: string;
  metrics: MetricInfo[];
}

export interface WindowSnapshot {
  used_percent: number | null;
  remaining_percent: number | null;
  resets_at: string | null;
  duration_minutes: number | null;
}

export interface AdditionalSnapshot {
  id: string;
  metric_id: string;
  label: string;
  window: WindowSnapshot;
}

export interface ProviderSnapshot {
  id: string;
  name: string;
  icon: string;
  brand_rgb: [number, number, number];
  account_name: string | null;
  plan_type: string | null;
  sampled_at: string;
  primary: WindowSnapshot;
  secondary: WindowSnapshot;
  additional: AdditionalSnapshot[];
}

export interface SnapshotResponse {
  type: "snapshot";
  ok: boolean;
  protocol: number;
  observed_at: string;
  executable: string;
  providers: ProviderSnapshot[];
}

export interface CatalogResponse {
  type: "catalog";
  ok: boolean;
  protocol: number;
  providers: ProviderInfo[];
}

type BridgeResponse = SnapshotResponse | CatalogResponse | { type: "accepted"; ok: boolean } | { type: "error"; ok: false; error: string };

export class BridgeUnavailableError extends Error {
  constructor(message = "Codex Minibar is not running") {
    super(message);
    this.name = "BridgeUnavailableError";
  }
}

function endpointPath(): string {
  const appData = process.env.APPDATA ?? path.join(process.env.USERPROFILE ?? ".", "AppData", "Roaming");
  return path.join(appData, "Codex Minibar", "Codex Minibar", "config", "streamdeck-bridge.json");
}

function fallbackExecutablePath(): string {
  const localAppData = process.env.LOCALAPPDATA ?? path.join(process.env.USERPROFILE ?? ".", "AppData", "Local");
  return path.join(localAppData, "Codex Minibar", "codex-minibar.exe");
}

async function loadEndpoint(): Promise<BridgeEndpoint> {
  try {
    const raw = await new Promise<string>((resolve, reject) => {
      readFile(endpointPath(), "utf8", (error, contents) => error ? reject(error) : resolve(contents));
    });
    const endpoint = JSON.parse(raw) as BridgeEndpoint;
    if (endpoint.protocol !== 1 || !endpoint.host || !endpoint.port || !endpoint.token) {
      throw new Error("invalid bridge endpoint");
    }
    return endpoint;
  } catch {
    throw new BridgeUnavailableError();
  }
}

function request(endpoint: BridgeEndpoint, payload: Record<string, unknown>): Promise<BridgeResponse> {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection({ host: endpoint.host, port: endpoint.port });
    let data = "";
    let settled = false;
    const finish = (callback: () => void) => {
      if (settled) return;
      settled = true;
      socket.destroy();
      callback();
    };
    socket.setTimeout(1500);
    socket.on("connect", () => {
      socket.write(`${JSON.stringify({ ...payload, token: endpoint.token })}\n`);
    });
    socket.on("data", chunk => {
      data += chunk.toString("utf8");
      const lineEnd = data.indexOf("\n");
      if (lineEnd < 0) return;
      const line = data.slice(0, lineEnd);
      try {
        finish(() => resolve(JSON.parse(line) as BridgeResponse));
      } catch (error) {
        finish(() => reject(error));
      }
    });
    socket.on("timeout", () => finish(() => reject(new BridgeUnavailableError("bridge request timed out"))));
    socket.on("error", () => finish(() => reject(new BridgeUnavailableError())));
    socket.on("close", () => {
      if (!settled) finish(() => reject(new BridgeUnavailableError()));
    });
  });
}

export class MinibarBridge {
  async snapshot(): Promise<SnapshotResponse> {
    const response = await request(await loadEndpoint(), { op: "snapshot" });
    if (response.type !== "snapshot" || !response.ok) {
      throw new BridgeUnavailableError("snapshot request failed");
    }
    return response;
  }

  async catalog(): Promise<CatalogResponse> {
    const response = await request(await loadEndpoint(), { op: "catalog" });
    if (response.type !== "catalog" || !response.ok) {
      throw new BridgeUnavailableError("catalog request failed");
    }
    return response;
  }

  async openPopup(provider?: string): Promise<void> {
    const response = await request(await loadEndpoint(), { op: "open_popup", provider: provider ?? null });
    if (response.type !== "accepted" || !response.ok) {
      throw new BridgeUnavailableError("popup request failed");
    }
  }

  async launchMinibar(): Promise<boolean> {
    let executable = fallbackExecutablePath();
    try {
      executable = (await loadEndpoint()).executable || executable;
    } catch {
      // The endpoint is expected to be absent in precisely this path.
    }
    if (!existsSync(executable)) return false;
    const child = spawn(executable, [], { detached: true, stdio: "ignore", windowsHide: true });
    child.unref();
    return true;
  }
}
