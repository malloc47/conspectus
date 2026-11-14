// Live state from an opencode 2.x client when no server entrypoint
// reports it (ADR 0229).
//
// The server entrypoint (`server.ts`) runs in the shared service and sees
// every session, but opencode loads it only for a package listed in
// `opencode.jsonc`. A package listed only in `cli.json`, as plugin 0.2.0
// asked, loads the TUI entrypoint alone. Each client still receives the
// service's whole event stream, so the TUI entrypoint can run the same
// reducer (`../live.ts`) over it and report the sessions its client has
// shown: the routed session, its family, and their descendants, from the
// moment the client routes to them until it closes. Sessions no client
// has shown are missed, and so is anything after the last client
// showing a session closes.
//
// Two writers for one session would replace each other's entries in the
// store, so the fallback reports only while the service says this
// plugin's server entrypoint is not active (its plugin list, which the
// entrypoint asks for through opencode's client), and says nothing until
// it has heard. When a session joins, or the fallback turns on, what the
// client's stores list of it (its turn's status, pending permissions and
// forms) fills in what the events before then did not say.
//
// The module is pure, like `../live.ts`: it imports only types, does
// nothing when loaded, and its tests run it without a build.

import type { HookPayload, LiveReducer } from "../live.js";

/** The id this plugin's entrypoints declare to opencode. */
export const PLUGIN_ID = "conspectus-hook";

/** What the client's stores list of a session; each answer is read as
 * {@link LiveReducer.seed} reads it, and a call that throws counts as no
 * answer. */
export type ClientStores = {
  status(sessionID: string): unknown;
  permissions(sessionID: string): unknown;
  forms(sessionID: string): unknown;
  /** The ids of the session's family: its root and every descendant. */
  family(sessionID: string): unknown;
};

export type TuiFallback = {
  /** An event of the client's stream (`ctx.data.listen`'s `details`). */
  event(details: unknown, now: number): HookPayload[];
  /** The session the client shows, or none. */
  route(sessionID: unknown, now: number): HookPayload[];
  /** The service's plugin list (`ctx.client.plugin.list`'s answer), or
   * `undefined` when the call failed. */
  plugins(answer: unknown, now: number): HookPayload[];
  heartbeat(now: number): HookPayload[];
  /** The client is going away. Its sessions go on in the service, so
   * nothing is sent. */
  dispose(now: number): HookPayload[];
  needsHeartbeat(): boolean;
  /** Whether the fallback reports: `undefined` until the service has
   * said whether a server entrypoint does. */
  reporting(): boolean | undefined;
};

function record(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

/** Whether the service's plugin list has this plugin's server
 * entrypoint active: an entry with its id, a server entrypoint, and the
 * status `active`. `undefined` for an answer that is not a list. Only
 * those three fields are read, never an error's text. */
export function serverReports(answer: unknown): boolean | undefined {
  const data = Array.isArray(answer) ? answer : record(answer)?.data;
  if (!Array.isArray(data)) return undefined;
  return data.some((entry) => {
    const plugin = record(entry);
    return (
      plugin?.id === PLUGIN_ID &&
      record(plugin.features)?.server === true &&
      record(plugin.state)?.status === "active"
    );
  });
}

export function createTuiFallback(live: LiveReducer, stores: ClientStores): TuiFallback {
  let reporting: boolean | undefined;
  let routed: string | undefined;
  /** The sessions the client has shown and their families. */
  const shown = new Set<string>();

  const covers = (sessionID: string) =>
    shown.has(sessionID) || live.lineage(sessionID).some((ancestor) => shown.has(ancestor));

  const keep = (out: HookPayload[]) => (reporting ? out.filter((payload) => covers(payload.session_id)) : []);

  const ask = (read: (sessionID: string) => unknown, sessionID: string): unknown => {
    try {
      return read(sessionID);
    } catch {
      return undefined;
    }
  };

  /** Fill `ids` in from the stores, then send the current state of every
   * session they cover that the filling did not already send. */
  function join(ids: Iterable<string>, name: string, now: number): HookPayload[] {
    const joined = new Set(ids);
    const out: HookPayload[] = [];
    for (const sessionID of joined) {
      out.push(
        ...live.seed(
          sessionID,
          {
            status: ask((id) => stores.status(id), sessionID),
            permissions: ask((id) => stores.permissions(id), sessionID),
            forms: ask((id) => stores.forms(id), sessionID),
          },
          now,
        ),
      );
    }
    const sent = new Set(out.map((payload) => payload.session_id));
    const within = (sessionID: string) =>
      joined.has(sessionID) || live.lineage(sessionID).some((ancestor) => joined.has(ancestor));
    out.push(...live.resend((sessionID) => !sent.has(sessionID) && within(sessionID), name, now));
    return keep(out);
  }

  function route(sessionID: unknown, now: number): HookPayload[] {
    if (typeof sessionID !== "string" || sessionID === routed) return [];
    routed = sessionID;
    const family = ask((id) => stores.family(id), sessionID);
    const joined = [sessionID, ...(Array.isArray(family) ? family.filter((id) => typeof id === "string") : [])].filter(
      (id) => !shown.has(id),
    );
    for (const id of joined) shown.add(id);
    return reporting ? join(joined, "tui.route", now) : [];
  }

  function plugins(answer: unknown, now: number): HookPayload[] {
    const server = serverReports(answer);
    // A failed check changes nothing: quiet until the service has said.
    if (server === undefined) return [];
    const was = reporting;
    reporting = !server;
    return reporting && !was ? join(shown, "tui.fallback", now) : [];
  }

  return {
    event: (details, now) => keep(live.v2(details, now)),
    route,
    plugins,
    heartbeat: (now) => keep(live.heartbeat(now)),
    dispose: () => [],
    needsHeartbeat: () => reporting === true && live.needsHeartbeat(),
    reporting: () => reporting,
  };
}
