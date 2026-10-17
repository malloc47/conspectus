// conspectus-hook: installed by `conspectus hook init pi`; `conspectus hook remove pi` deletes this file.
//
// Pi extension that tells Conspectus which session a Pi process is running
// (ADR 0121). On every `session_start` in the interactive TUI it runs
// `conspectus hook write pi` with the session id, cwd, and session file.
// Conspectus records that against the tmux pane, so attribution follows
// `/new`, `/resume`, `/fork`, and `/clone` inside one running Pi.
//
// - `CONSPECTUS_HOOK_BIN` picks the binary; otherwise `conspectus` is
//   found on PATH.
// - A missing binary or a failed write is ignored, so Pi is never
//   disturbed.
// - `--no-session` runs and the print, JSON, and RPC modes write nothing:
//   there is no session file, or no pane, to attribute.

import { spawn } from "node:child_process";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

// Recorded on each hook record so it's clear which extension wrote it.
const EXTENSION_VERSION = "1";

export default function (pi: ExtensionAPI) {
  pi.on("session_start", async (event, ctx) => {
    if (ctx.mode !== "tui") {
      return;
    }
    const sessionFile = ctx.sessionManager.getSessionFile();
    if (!sessionFile) {
      return;
    }
    const payload = JSON.stringify({
      session_id: ctx.sessionManager.getSessionId(),
      cwd: ctx.sessionManager.getCwd() ?? ctx.cwd,
      transcript_path: sessionFile,
      hook_event_name: `session_start:${event.reason}`,
    });
    try {
      const child = spawn(
        process.env.CONSPECTUS_HOOK_BIN || "conspectus",
        ["hook", "write", "pi"],
        {
          stdio: ["pipe", "ignore", "ignore"],
          env: { ...process.env, CONSPECTUS_PI_HOOK_VERSION: EXTENSION_VERSION },
        },
      );
      child.on("error", () => {});
      child.stdin.on("error", () => {});
      child.stdin.end(payload);
      child.unref();
    } catch {
      // Ignored: Pi keeps running without attribution.
    }
  });
}
