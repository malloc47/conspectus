// Conspectus opencode hook plugin for opencode 2.x (TUI entrypoint).
//
// opencode 2.x runs server plugins inside one shared service whose
// environment belongs to whichever client started it, so a server
// plugin cannot say which pane shows which session. A TUI plugin runs
// inside its own client's process instead: `conspectus hook write`
// spawned from here inherits this pane's tmux context and finds this
// client as its harness process, just as the 1.x plugin did from inside
// each TUI (ADR 0126).
//
// The plugin forwards the session this client is showing: once whenever
// the client's route moves to a session, including in-app switches, and
// on that session's lifecycle events. Events for other sessions reach
// every client, so they are ignored here; forwarding them would stamp
// this pane onto sessions it isn't showing.

import type { Plugin } from "@opencode/plugin/tui";
import { writeHook } from "../write.js";

// 2.x equivalents of the 1.x lifecycle events the 1.x plugin forwards
// (`session.renamed` stands in for 1.x `session.updated`).
const LIFECYCLE_EVENTS = new Set([
  "session.created",
  "session.renamed",
  "session.status",
  "session.idle",
  "session.compacted",
]);

// The router has no change event, so route changes are noticed on any
// event and on this poll, whichever comes first.
const ROUTE_POLL_MS = 1000;

type Route = { readonly type: string; readonly sessionID?: string };

function routedSession(route: Route): string | undefined {
  return route.type === "session" ? route.sessionID : undefined;
}

function eventSession(details: unknown): string | undefined {
  if (typeof details !== "object" || details === null) return undefined;
  const data = (details as { data?: unknown }).data;
  if (typeof data !== "object" || data === null) return undefined;
  const sessionID = (data as { sessionID?: unknown }).sessionID;
  return typeof sessionID === "string" ? sessionID : undefined;
}

// Typed against `Plugin.Definition` rather than built with
// `Plugin.define` (an identity function) so the plugin has no runtime
// dependency to resolve.
const plugin: Plugin.Definition = {
  id: "conspectus-hook",
  setup(ctx) {
    const cwd = ctx.location?.directory;
    let current: string | undefined;

    const checkRoute = () => {
      const session = routedSession(ctx.ui.router.current());
      if (session === current) return;
      current = session;
      if (session) {
        writeHook({ session_id: session, cwd, hook_event_name: "tui.route" });
      }
    };

    const stopListening = ctx.data.listen(({ details }) => {
      checkRoute();
      if (!LIFECYCLE_EVENTS.has(details.type)) return;
      const session = eventSession(details);
      if (session && session === current) {
        writeHook({ session_id: session, cwd, hook_event_name: details.type });
      }
    });
    const timer = setInterval(checkRoute, ROUTE_POLL_MS);
    checkRoute();

    return () => {
      stopListening();
      clearInterval(timer);
    };
  },
};

export default plugin;
