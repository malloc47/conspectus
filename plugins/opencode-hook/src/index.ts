// Conspectus opencode hook plugin for opencode 1.x.
//
// Subscribes to opencode session lifecycle events and forwards a flat
// payload to `conspectus hook write opencode` so Conspectus discovery
// can attribute live mux panes to the operator's current opencode
// session even after in-app `/session resume` switches. See
// `docs/adr/0049-opencode-hook-plugin-distribution.md` for the
// distribution and shape contract. opencode 2.x loads `v2/tui.ts`
// instead (ADR 0126).

import type { Plugin } from "@opencode-ai/plugin";
import { writeHook } from "./write.js";

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
