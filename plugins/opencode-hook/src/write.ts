// Spawns `conspectus hook write opencode` with one flat payload. Shared
// by the opencode 1.x plugin (`index.ts`) and the 2.x TUI entrypoint
// (`v2/tui.ts`).

import { spawn } from "node:child_process";

export const PLUGIN_VERSION = "0.2.0";
const CONSPECTUS_BIN = process.env.CONSPECTUS_HOOK_BIN ?? "conspectus";

// Flat payload accepted by `conspectus hook write opencode`. Keep this
// in sync with `HarnessAdapter::hook_record_from_payload` on the Rust
// side.
export type HookPayload = {
  session_id: string;
  cwd?: string;
  hook_event_name?: string;
};

let spawnWarningShown = false;

export function writeHook(payload: HookPayload): void {
  let child;
  try {
    child = spawn(CONSPECTUS_BIN, ["hook", "write", "opencode"], {
      stdio: ["pipe", "ignore", "ignore"],
      env: {
        ...process.env,
        CONSPECTUS_OPENCODE_HOOK_VERSION: PLUGIN_VERSION,
      },
    });
  } catch (err) {
    warnOnce(err);
    return;
  }

  child.on("error", (err) => {
    warnOnce(err);
  });
  child.stdin.on("error", () => {
    // Writer exited before we could finish piping. Nothing to do — the
    // record either landed or didn't; we don't retry.
  });
  try {
    child.stdin.write(JSON.stringify(payload));
    child.stdin.end();
  } catch (err) {
    warnOnce(err);
  }
}

function warnOnce(err: unknown): void {
  if (spawnWarningShown) return;
  spawnWarningShown = true;
  const detail = err instanceof Error ? err.message : String(err);
  process.stderr.write(
    `[conspectus] opencode hook plugin could not spawn '${CONSPECTUS_BIN} hook write opencode': ${detail}. ` +
      `Session attribution via Conspectus will fall back to other evidence. ` +
      `Install the conspectus binary on PATH or set CONSPECTUS_HOOK_BIN to suppress this warning.\n`,
  );
}
