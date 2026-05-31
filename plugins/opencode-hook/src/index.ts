// Conspectus opencode hook plugin.
//
// Subscribes to opencode session lifecycle events and forwards a flat
// payload to `conspectus hook write opencode` so Conspectus discovery
// can attribute live mux panes to the operator's current opencode
// session even after in-app `/session resume` switches. See
// `docs/adr/0049-opencode-hook-plugin-distribution.md` for the
// distribution and shape contract.

import { spawn } from "node:child_process";
import type { Plugin } from "@opencode-ai/plugin";

const PLUGIN_VERSION = "0.1.0";
const CONSPECTUS_BIN = process.env.CONSPECTUS_HOOK_BIN ?? "conspectus";

// Flat payload accepted by `conspectus hook write opencode`. Keep this
// in sync with `src/hook.rs::opencode_record_from_payload` on the Rust
// side.
type HookPayload = {
  session_id: string;
  cwd?: string;
  hook_event_name?: string;
};

let spawnWarningShown = false;

function writeHook(payload: HookPayload): void {
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

export const ConspectusOpencodeHook: Plugin = async (input) => {
  const fallbackCwd: string | undefined = input.directory ?? input.worktree;

  return {
    event: async ({ event }) => {
      let session_id: string | undefined;
      let cwd: string | undefined = fallbackCwd;

      switch (event.type) {
        case "session.created":
        case "session.updated":
          session_id = event.properties.info.id;
          cwd = event.properties.info.directory ?? fallbackCwd;
          break;
        case "session.status":
        case "session.idle":
        case "session.compacted":
          session_id = event.properties.sessionID;
          break;
        default:
          return;
      }

      if (!session_id) return;

      writeHook({
        session_id,
        cwd,
        hook_event_name: event.type,
      });
    },
  };
};

export default ConspectusOpencodeHook;
