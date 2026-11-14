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
// this pane onto sessions it isn't showing. These records carry no live
// state, so Conspectus records them for attribution only.
//
// Each session's state comes from the server entrypoint (`server.ts`),
// which sees every session, when opencode loads it: for a package listed
// in `opencode.jsonc`. Listed only in `cli.json`, this entrypoint loads
// alone, and then reports the state of the sessions its client has shown
// itself (`fallback.ts`, ADR 0229): it asks the service, through
// opencode's client, whether this plugin's server entrypoint is active,
// and stays quiet while it is.

import type { Plugin } from "@opencode/plugin/tui";
import { createLiveState, newInstanceId } from "../live.js";
import { driveLiveState, ownStartTime, writeHook } from "../write.js";
import { createTuiFallback, type ClientStores } from "./fallback.js";

// 2.x equivalents of the 1.x lifecycle events the 1.x plugin forwards
// (`session.renamed` stands in for 1.x `session.updated`). 2.x has no
// `session.compacted`; its `session.compaction.*` events say nothing
// about which session a pane shows.
const LIFECYCLE_EVENTS = new Set([
  "session.created",
  "session.renamed",
  "session.status",
  "session.idle",
]);

// The router has no change event, so route changes are noticed on any
// event and on this poll, whichever comes first.
const ROUTE_POLL_MS = 1000;

/** How often the client asks the service whether this plugin's server
 * entrypoint is active. */
const SERVER_CHECK_MS = 60_000;

type Route = { readonly type: string; readonly sessionID?: string };

/** The parts of the context the fallback reads, typed loosely so that a
 * version without one still loads, and does without it. */
type Loose = {
  readonly data?: {
    readonly session?: {
      status?(sessionID: string): unknown;
      family?(sessionID: string): unknown;
      readonly permission?: { list?(sessionID: string): unknown };
      readonly form?: { list?(sessionID: string): unknown };
    };
  };
  readonly client?: { readonly plugin?: { list?(input: unknown): unknown } };
};

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

/** The client's session stores, as the fallback reads them. */
function stores(ctx: Loose): ClientStores {
  const session = ctx.data?.session;
  return {
    status: (sessionID) => session?.status?.(sessionID),
    family: (sessionID) => session?.family?.(sessionID),
    permissions: (sessionID) => session?.permission?.list?.(sessionID),
    forms: (sessionID) => session?.form?.list?.(sessionID),
  };
}

// Typed against `Plugin.Definition` rather than built with
// `Plugin.define` (an identity function) so the plugin has no runtime
// dependency to resolve.
const plugin: Plugin.Definition = {
  id: "conspectus-hook",
  setup(ctx) {
    const cwd = ctx.location?.directory;
    const loose = ctx as unknown as Loose;
    const live = driveLiveState(
      createTuiFallback(
        createLiveState({ instance: newInstanceId(), pid: process.pid, pidStart: ownStartTime(), directory: cwd }),
        stores(loose),
      ),
    );
    let current: string | undefined;

    const checkRoute = () => {
      const session = routedSession(ctx.ui.router.current());
      if (session === current) return;
      current = session;
      if (session) {
        writeHook({ session_id: session, cwd, hook_event_name: "tui.route" });
        live.send((fallback, now) => fallback.route(session, now));
      }
    };

    let checking = false;
    const checkServer = async () => {
      if (checking) return;
      checking = true;
      let answer: unknown;
      try {
        answer = await loose.client?.plugin?.list?.(cwd === undefined ? {} : { location: { directory: cwd } });
      } catch {
        answer = undefined;
      }
      checking = false;
      live.send((fallback, now) => fallback.plugins(answer, now));
    };

    const stopListening = ctx.data.listen(({ details }) => {
      checkRoute();
      live.send((fallback, now) => fallback.event(details, now));
      if (!LIFECYCLE_EVENTS.has(details.type)) return;
      const session = eventSession(details);
      if (session && session === current) {
        writeHook({ session_id: session, cwd, hook_event_name: details.type });
      }
    });
    const routeTimer = setInterval(checkRoute, ROUTE_POLL_MS);
    const serverTimer = setInterval(() => void checkServer(), SERVER_CHECK_MS);
    // The check never keeps opencode running.
    serverTimer.unref?.();
    checkRoute();
    void checkServer();

    return () => {
      stopListening();
      clearInterval(routeTimer);
      clearInterval(serverTimer);
      live.dispose();
    };
  },
};

export default plugin;
