// Spawns `conspectus hook write opencode` with one flat payload, and
// drives the live-state reducer (`live.ts`): its snapshots go out on
// every change, and again every heartbeat while a session is not ready.
// Shared by the opencode 1.x plugin (`index.ts`) and the 2.x entrypoints
// (`v2/server.ts`, `v2/tui.ts`).
//
// Nothing here may disturb opencode: a write is a detached process that
// is never awaited and never keeps opencode running, and every failure
// is swallowed (ADR 0049, ADR 0206 decision 13).

import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { HEARTBEAT_S, parseProcStartTime, type HookPayload, type LiveReducer } from "./live.js";

export type { HookPayload } from "./live.js";

export const PLUGIN_VERSION = "0.3.0";
const CONSPECTUS_BIN = process.env.CONSPECTUS_HOOK_BIN ?? "conspectus";

let spawnWarningShown = false;

export function writeHook(payload: HookPayload): void {
  let child;
  try {
    // Detached, so a write still lands when opencode exits right after
    // it (a quit sends the dispose snapshots), and unref'd, so a write
    // in flight never holds opencode's exit.
    child = spawn(CONSPECTUS_BIN, ["hook", "write", "opencode"], {
      stdio: ["pipe", "ignore", "ignore"],
      detached: true,
      windowsHide: true,
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
    child.unref();
    child.stdin.write(JSON.stringify(payload));
    child.stdin.end();
  } catch (err) {
    warnOnce(err);
  }
}

/** This process's start time as `/proc/self/stat` gives it, the value
 * Conspectus compares to tell a reused pid; none off Linux. */
export function ownStartTime(): number | undefined {
  try {
    return parseProcStartTime(readFileSync("/proc/self/stat", "utf8"));
  } catch {
    return undefined;
  }
}

export type LiveDriver = {
  /** Apply one input to the reducer at the current time and send what it
   * emits. */
  send(step: (live: LiveReducer, now: number) => HookPayload[]): void;
  /** The instance is going away: send the last snapshots and stop the
   * heartbeat. */
  dispose(): void;
};

/** Run `live`: send its snapshots, and keep a heartbeat timer only while
 * a session is due one. */
export function driveLiveState(live: LiveReducer): LiveDriver {
  let timer: ReturnType<typeof setInterval> | undefined;
  let disposed = false;

  const stopTimer = () => {
    if (timer !== undefined) clearInterval(timer);
    timer = undefined;
  };

  const run = (step: (live: LiveReducer, now: number) => HookPayload[]) => {
    try {
      for (const payload of step(live, Date.now())) writeHook(payload);
      if (disposed || !live.needsHeartbeat()) {
        stopTimer();
      } else if (timer === undefined) {
        timer = setInterval(() => run((state, now) => state.heartbeat(now)), HEARTBEAT_S * 1000);
        // The heartbeat never keeps opencode running.
        timer.unref?.();
      }
    } catch {
      // A reducer bug must not reach opencode; the next event repairs
      // the snapshot.
    }
  };

  return {
    send(step) {
      if (!disposed) run(step);
    },
    dispose() {
      if (disposed) return;
      disposed = true;
      run((state, now) => state.dispose(now));
      stopTimer();
    },
  };
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
