// Conspectus opencode hook plugin for opencode 1.x.
//
// Runs inside each TUI and hands opencode's session events to the
// live-state reducer (`live.ts`), which sends each session's state to
// `conspectus hook write opencode` whenever it changes (ADR 0206). The
// writer adds the pane's tmux context and this process. A root
// session's lifecycle events (created, updated, status, idle, compacted)
// are also recorded for attribution, so Conspectus can tell which session
// a pane shows even after an in-app `/session resume` (ADR 0049); a child
// session's never are, since the pane still shows its parent. opencode
// 2.x loads `v2/server.ts` and `v2/tui.ts` instead (ADR 0126).

import type { Hooks, Plugin } from "@opencode-ai/plugin";
import { createLiveState, newInstanceId } from "./live.js";
import { driveLiveState, ownStartTime } from "./write.js";

export const ConspectusOpencodeHook: Plugin = async (input) => {
  const live = driveLiveState(
    createLiveState({
      instance: newInstanceId(),
      pid: process.pid,
      pidStart: ownStartTime(),
      directory: input.directory ?? input.worktree,
    }),
  );

  // opencode 1.18 calls `dispose` when the instance closes; the typed
  // hooks of older plugin packages do not declare it.
  const hooks: Hooks & { dispose?: () => Promise<void> } = {
    event: async ({ event }) => {
      live.send((state, now) => state.v1(event, now));
    },
    "experimental.session.compacting": async ({ sessionID }) => {
      live.send((state, now) => state.compacting(sessionID, now));
    },
    dispose: async () => {
      live.dispose();
    },
  };
  return hooks;
};

export default ConspectusOpencodeHook;
