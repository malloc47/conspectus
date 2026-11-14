// Live state for the Conspectus opencode plugin (ADR 0206 decision 3).
//
// opencode publishes what a session is doing as plugin events: busy,
// idle, and retry statuses, permission asks, questions and forms, errors,
// compaction, and child sessions. No storage poll can see a pending ask,
// so the plugin keeps a small state per session here and, whenever what a
// session shows changes, sends the whole of it as a snapshot through
// `conspectus hook write opencode`. The snapshot replaces the session's
// entry in Conspectus's live-state store, ordered by this instance's id
// and a sequence number (ADR 0204 decision 3), so it never depends on an
// earlier one: a lost or reordered write is healed by the next.
//
// The module is pure. It imports nothing, does nothing when loaded, and
// uses only TypeScript that Node can strip, so `node --test` runs it
// without a build (`test/live.test.ts`, which replays the event scripts
// under `tests/fixtures/hooks/opencode-live/`). The entrypoints hand it
// events and send what it returns.
//
// What it keeps per session, and what each event does to it:
//
// | 1.x event                                 | 2.x event                                   | Effect |
// | :-                                        | :-                                          | :- |
// | `session.created` (`info.parentID`)       | `session.created` (`parentID`)              | knows the session and its parent; a new one is ready |
// | `session.status` busy                     | `session.execution.started`                 | working (generating) |
// | `session.status` retry                    | `session.retry.scheduled`                   | working (retrying, or rate_limited) |
// | the next busy                             | `session.step.started` after a retry        | working (generating) |
// | `session.status` idle, `session.idle`     | `session.execution.succeeded`               | ready (finished), waits cleared |
// | `session.error`                           | `session.execution.failed`                  | error (its class); 1.x shows it at the idle that ends the turn |
// |                                           | `session.execution.interrupted`             | ready (interrupted); `shutdown` ends the session |
// | compaction hook, `session.compacted`      | `session.compaction.started`, `.ended`, `.failed` | compacting, then the state it interrupted |
// | `permission.asked`, `permission.replied`  | the same                                    | a pending permission; a `reject` clears the session's |
// | `question.asked`, `.replied`, `.rejected` | `form.created` of kind `question`, `form.replied`, `form.cancelled` | a pending question |
// |                                           | any other `form.created`                    | pending input |
// | `session.deleted`                         | `session.deleted`, `session.archived`       | a last snapshot, exited (ended) |
// | `dispose`                                 | the plugin unloading                        | every session still working or waiting exits (ended) |
// |                                           | asks listed when the server plugin starts   | the pending permissions it never saw (see {@link LiveReducer.seed}) |
//
// What a session shows, first match wins (research note 03, section 7.2,
// in the vocabulary of ADR 0203): exited when it ended; waiting on its
// own permission, question, or input; waiting on a descendant's, raised
// with `via_subagent`; compacting; working; error; ready. `subagents`
// counts the descendants that are working, waiting, or compacting.
// `since_ms` is when that began: a wait shows its own start, and once it
// ends the session's own state shows again with the start it had, as
// does the state compaction interrupted. A session nothing has said
// anything about shows nothing, which is no evidence, never ready.
//
// Privacy (ADR 0086 Tier 3, ADR 0206 decisions 10 to 12): only ids,
// paths, closed names, counts, and times leave this module. It reads an
// event's type, its session, request, form, and parent ids, a session's
// directory, a status's type and whether a retry has a reason, a reply
// word, a form's kind, an interruption's reason, and an error's name,
// type, status, and retry delay, and reads the words only to classify
// them. It never reads a title, a message, a permission's patterns,
// resources, or metadata, a question, a form's fields, or an error's
// text, and an id that does not look like one is refused. What opencode
// lists of a session ({@link LiveReducer.seed}, {@link readPending}) is
// read the same way: a session's and an ask's id and session, a form's
// kind, and a turn's status word.

/** Seconds between heartbeats of a session that is not ready. */
export const HEARTBEAT_S = 60;

/** The most sessions one instance remembers; the least recently changed
 * idle ones go first. */
export const MAX_SESSIONS = 256;

/** The opencode 1.x events the plugin hands to {@link LiveReducer.v1}. */
export const V1_EVENTS = [
  "session.created",
  "session.updated",
  "session.deleted",
  "session.status",
  "session.idle",
  "session.error",
  "session.compacted",
  "permission.asked",
  "permission.updated",
  "permission.replied",
  "question.asked",
  "question.replied",
  "question.rejected",
] as const;

/** The opencode 2.x events the server entrypoint subscribes to. */
export const V2_EVENTS = [
  "session.created",
  "session.deleted",
  "session.archived",
  "session.execution.started",
  "session.execution.succeeded",
  "session.execution.failed",
  "session.execution.interrupted",
  "session.retry.scheduled",
  "session.step.started",
  "session.compaction.started",
  "session.compaction.ended",
  "session.compaction.failed",
  "session.status",
  "permission.asked",
  "permission.replied",
  "form.created",
  "form.replied",
  "form.cancelled",
] as const;

/** The most sessions {@link readPending} asks about when the server
 * plugin starts: the location's most recent ones. */
export const MAX_LISTED_SESSIONS = 100;

/** Sessions {@link readPending} asks for at a time. */
const LIST_PAGE = 50;

/** How many ended asks an instance remembers, so a list read before an
 * answer cannot bring the ask back. */
const MAX_ANSWERED = 512;

/** The 1.x events that also name the session a pane shows, so a root
 * session's are recorded for attribution as they were before live
 * state. A child session's never are. */
const ATTRIBUTION_V1 = new Set<string>([
  "session.created",
  "session.updated",
  "session.status",
  "session.idle",
  "session.compacted",
]);

export type LiveState =
  | "working"
  | "waiting"
  | "ready"
  | "compacting"
  | "error"
  | "exited"
  | "unknown";

export type LiveDetail =
  | "generating"
  | "retrying"
  | "rate_limited"
  | "permission"
  | "question"
  | "input"
  | "finished"
  | "interrupted"
  | "auth"
  | "overloaded"
  | "failed"
  | "ended";

/** One session's whole state, as `conspectus hook write opencode` reads
 * it from a payload's `live` object. */
export type LiveSnapshot = {
  /** This plugin instance: a fresh id each time the plugin loads. */
  instance: string;
  /** Counts up with every snapshot the instance sends. */
  seq: number;
  /** When the snapshot was taken, Unix epoch milliseconds. */
  observed_ms: number;
  heartbeat_s: number;
  /** The opencode process. */
  pid?: number;
  /** Its start time, field 22 of `/proc/<pid>/stat`. */
  pid_start?: number;
  state: LiveState;
  detail?: LiveDetail;
  subagents: number;
  via_subagent: boolean;
  /** When the state began. */
  since_ms: number;
};

/** The flat payload `conspectus hook write opencode` reads on stdin. A
 * payload without `live` is recorded for attribution only, as plugin
 * 0.2.0 sent it; one with `live` is recorded for attribution as well
 * only when `attribution` is true. */
export type HookPayload = {
  session_id: string;
  cwd?: string;
  hook_event_name?: string;
  attribution?: boolean;
  live?: LiveSnapshot;
};

export type LiveOptions = {
  /** From {@link newInstanceId}. */
  instance: string;
  pid?: number;
  pidStart?: number;
  /** The directory the plugin instance serves: the cwd of a session that
   * names none, and on 2.x the location whose events it keeps. */
  directory?: string;
  heartbeatS?: number;
};

/** What opencode lists of one session, as {@link LiveReducer.seed} reads
 * it. Every field is optional and anything it does not recognize is
 * skipped. */
export type Listed = {
  /** The session's turn: `running` or `idle`. */
  status?: unknown;
  /** Its pending permission requests, each `{id, sessionID}`. */
  permissions?: unknown;
  /** Its pending forms, each `{id, sessionID, metadata: {kind}}`. */
  forms?: unknown;
};

export type LiveReducer = {
  /** An opencode 1.x `event` hook event. */
  v1(event: unknown, now: number): HookPayload[];
  /** An opencode 2.x event, as the server's event stream yields it. */
  v2(event: unknown, now: number): HookPayload[];
  /** The 1.x `experimental.session.compacting` hook. */
  compacting(sessionID: unknown, now: number): HookPayload[];
  /** The snapshots due a heartbeat: every session that is not ready. */
  heartbeat(now: number): HookPayload[];
  /** The instance is going away: what is still working or waiting ends. */
  dispose(now: number): HookPayload[];
  /** What opencode lists of a session the events may not have told:
   * pending asks the instance never saw are added, and the turn's status
   * fills in a session it has no evidence for. Nothing is removed, and an
   * ask the instance saw end is not brought back. */
  seed(sessionID: unknown, listed: Listed, now: number): HookPayload[];
  /** The current snapshot of every session `include` accepts that shows
   * something, sent again whether or not it changed: for a source that
   * starts reporting sessions it already follows. */
  resend(include: (sessionID: string) => boolean, name: string, now: number): HookPayload[];
  /** The ids of a session's ancestors, nearest first, as far as the
   * instance has heard of them. */
  lineage(sessionID: string): string[];
  /** Whether any session is due heartbeats. */
  needsHeartbeat(): boolean;
};

type BaseState = "working" | "ready" | "compacting" | "error" | "exited";
type WaitKind = "permission" | "question" | "input";
type Failure = "rate_limited" | "auth" | "overloaded" | "failed";

/** A state with its detail and when it began. */
type Phase = { state: BaseState; detail?: LiveDetail; since: number };

type Wait = { kind: WaitKind; since: number };

/** What a session shows, as its last snapshot said it. */
type View = {
  state: LiveState;
  detail?: LiveDetail;
  since: number;
  subagents: number;
  via: boolean;
  cwd?: string;
};

type Session = {
  id: string;
  parent?: string;
  cwd?: string;
  /** The session's own state, under any wait; none before any evidence. */
  base?: Phase;
  /** The state to resume when compaction ends. */
  resume?: Phase;
  /** 1.x: how the running turn ends when its idle arrives. */
  ending?: Failure | "interrupted";
  /** Pending asks by `<kind>:<id>`. */
  waits: Map<string, Wait>;
  shown?: View;
  /** When an event last touched the session. */
  changed: number;
};

const WAIT_ORDER: readonly WaitKind[] = ["permission", "question", "input"];

/** States that age without evidence, so their snapshot is resent. */
const BEATING = new Set<LiveState>(["working", "waiting", "compacting", "error"]);

const MAX_ID_LEN = 256;
const MAX_PATH_LEN = 4096;

function record(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

/** `value` when it looks like an id: a short string with no whitespace
 * or control character, as the store requires. */
function id(value: unknown): string | undefined {
  return typeof value === "string" &&
    value.length > 0 &&
    value.length <= MAX_ID_LEN &&
    !/[\s\p{Cc}]/u.test(value)
    ? value
    : undefined;
}

function path(value: unknown): string | undefined {
  return typeof value === "string" &&
    value.length > 0 &&
    value.length <= MAX_PATH_LEN &&
    !/\p{Cc}/u.test(value)
    ? value
    : undefined;
}

/** A word to classify on, lowercased; it is never copied anywhere. */
function word(value: unknown): string | undefined {
  return typeof value === "string" ? value.toLowerCase() : undefined;
}

function number(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

/** A location's directory: the location itself when it is a string. */
function directoryOf(location: unknown): string | undefined {
  return typeof location === "string" ? path(location) : path(record(location)?.directory);
}

function sameDirectory(a: string, b: string): boolean {
  const trim = (dir: string) => (dir.length > 1 ? dir.replace(/\/+$/, "") : dir);
  return trim(a) === trim(b);
}

/** A failure by its HTTP status, when that says one. */
function failureOfStatus(status: number | undefined): Failure | undefined {
  if (status === 429) return "rate_limited";
  if (status === 401 || status === 403) return "auth";
  if (status === 503 || status === 529) return "overloaded";
  return undefined;
}

/** How a 1.x `session.error` ends the turn, from the error's name and,
 * for an API error, its status code. */
function endingOfV1Error(error: Record<string, unknown> | undefined): Failure | "interrupted" {
  const name = word(error?.name);
  if (name === "messageabortederror") return "interrupted";
  if (name === "providerautherror") return "auth";
  return failureOfStatus(number(record(error?.data)?.statusCode)) ?? "failed";
}

/** A 2.x execution failure's class, from its status, retry delay, and
 * type. */
function failureOfV2Error(error: Record<string, unknown> | undefined): Failure {
  const byStatus = failureOfStatus(number(error?.status));
  if (byStatus) return byStatus;
  if (number(error?.retryAfterMs) !== undefined) return "rate_limited";
  const type = word(error?.type) ?? "";
  if (type.includes("auth")) return "auth";
  if (type.includes("overload")) return "overloaded";
  if (type.includes("rate")) return "rate_limited";
  return "failed";
}

/** Whether a 2.x retry is held back by a rate limit: a 429, or a delay
 * the provider asked for. */
function rateLimitedV2(error: Record<string, unknown> | undefined): boolean {
  return number(error?.status) === 429 || number(error?.retryAfterMs) !== undefined;
}

/** The start time of a process, field 22 of a `/proc/<pid>/stat` line,
 * counted past the command name in parentheses (which may hold spaces
 * or parentheses itself), as Conspectus reads it. */
export function parseProcStartTime(stat: string): number | undefined {
  const close = stat.lastIndexOf(")");
  if (close < 0) return undefined;
  const fields = stat.slice(close + 1).trim().split(/\s+/);
  const start = fields[19];
  return start !== undefined && /^\d+$/.test(start) ? Number(start) : undefined;
}

/** A fresh instance id: the time and 64 random bits, which is unique
 * enough to tell one plugin load from another. */
export function newInstanceId(now: number = Date.now(), random: () => number = Math.random): string {
  let bits = "";
  for (let i = 0; i < 4; i++) {
    bits += Math.floor(random() * 0x10000)
      .toString(16)
      .padStart(4, "0");
  }
  return `${Math.max(0, Math.floor(now)).toString(36)}-${bits}`;
}

/** The live state of one plugin instance's sessions. */
export function createLiveState(options: LiveOptions): LiveReducer {
  const sessions = new Map<string, Session>();
  const heartbeatS = options.heartbeatS ?? HEARTBEAT_S;
  const directory = path(options.directory);
  /** Asks seen to end, by `<kind>:<id>`, oldest first. */
  const answered = new Set<string>();
  let seq = 0;

  function ended(key: string): void {
    answered.delete(key);
    answered.add(key);
    if (answered.size > MAX_ANSWERED) {
      const oldest = answered.values().next();
      if (!oldest.done) answered.delete(oldest.value);
    }
  }

  /** Drop the session's pending asks, remembering that they ended. */
  function clearWaits(s: Session): void {
    for (const key of s.waits.keys()) ended(key);
    s.waits.clear();
  }

  function session(sessionID: string, now: number): Session {
    let found = sessions.get(sessionID);
    if (!found) {
      found = { id: sessionID, waits: new Map(), changed: now };
      sessions.set(sessionID, found);
    }
    found.changed = now;
    return found;
  }

  function existing(sessionID: string | undefined, now: number): Session | undefined {
    const found = sessionID === undefined ? undefined : sessions.get(sessionID);
    if (found) found.changed = now;
    return found;
  }

  /** Record what session info says: the parent and the directory. Returns
   * a former parent, whose children changed. */
  function learn(s: Session, parentID: unknown, dir: unknown): Session | undefined {
    const parent = id(parentID);
    const cwd = path(dir);
    if (cwd) s.cwd = cwd;
    if (!parent || parent === s.id || parent === s.parent) return undefined;
    const former = s.parent === undefined ? undefined : sessions.get(s.parent);
    s.parent = parent;
    return former;
  }

  /** Make `state` the session's own state. It keeps its start when the
   * state stays the same, and a planned resume is dropped once the
   * session no longer compacts. */
  function setBase(s: Session, state: BaseState, detail: LiveDetail | undefined, now: number): void {
    const since = s.base && s.base.state === state ? s.base.since : now;
    s.base = detail === undefined ? { state, since } : { state, detail, since };
    if (state !== "compacting") s.resume = undefined;
  }

  /** The session works. Leaving compaction for the turn it interrupted
   * keeps that turn's start. */
  function work(s: Session, detail: LiveDetail, now: number): void {
    if (s.base?.state === "compacting" && s.resume?.state === "working") {
      s.base = { state: "working", detail, since: s.resume.since };
      s.resume = undefined;
      return;
    }
    setBase(s, "working", detail, now);
  }

  function turnStarted(s: Session, now: number): void {
    s.ending = undefined;
    work(s, "generating", now);
  }

  /** 1.x: the turn is over. Its pending ending decides how it shows; a
   * repeated idle, or one after a failure already shown, changes
   * nothing. */
  function idle(s: Session, now: number): void {
    const state = s.base?.state;
    if (state !== undefined && state !== "working" && state !== "compacting") return;
    clearWaits(s);
    const ending = s.ending;
    s.ending = undefined;
    if (ending === "interrupted") setBase(s, "ready", "interrupted", now);
    else if (ending) setBase(s, "error", ending, now);
    else setBase(s, "ready", "finished", now);
  }

  function finish(s: Session, state: "ready" | "error", detail: LiveDetail, now: number): void {
    clearWaits(s);
    s.ending = undefined;
    setBase(s, state, detail, now);
  }

  function end(s: Session, now: number): void {
    clearWaits(s);
    s.ending = undefined;
    setBase(s, "exited", "ended", now);
  }

  function startCompaction(s: Session, now: number): void {
    if (s.base?.state === "compacting") return;
    const resume =
      s.base && (s.base.state === "working" || s.base.state === "ready") ? s.base : undefined;
    setBase(s, "compacting", undefined, now);
    s.resume = resume;
  }

  function endCompaction(s: Session, now: number): void {
    if (s.base?.state !== "compacting") return;
    s.base = s.resume ?? { state: "ready", since: now };
    s.resume = undefined;
  }

  /** A pending ask means a turn runs, whatever was seen before it. */
  function startWait(s: Session, kind: WaitKind, requestID: string, now: number): void {
    if (s.base?.state !== "working" && s.base?.state !== "compacting") work(s, "generating", now);
    const key = `${kind}:${requestID}`;
    if (!s.waits.has(key)) s.waits.set(key, { kind, since: now });
  }

  /** Remove the asks `keys` name from the session, or from whichever
   * session holds them when it does not. */
  function endWaits(sessionID: string | undefined, keys: string[], now: number): Session[] {
    for (const key of keys) ended(key);
    const holds = (s: Session) => keys.some((key) => s.waits.has(key));
    const own = existing(sessionID, now);
    const holders = own && holds(own) ? [own] : [...sessions.values()].filter(holds);
    for (const s of holders) {
      for (const key of keys) s.waits.delete(key);
      s.changed = now;
    }
    return holders;
  }

  function reply(sessionID: string | undefined, requestID: unknown, answer: unknown, now: number): Session[] {
    const request = id(requestID);
    if (word(answer) === "reject") {
      if (request) ended(`permission:${request}`);
      const s = existing(sessionID, now);
      if (!s) return [];
      for (const [key, wait] of s.waits) {
        if (wait.kind !== "permission") continue;
        ended(key);
        s.waits.delete(key);
      }
      return [s];
    }
    return request ? endWaits(sessionID, [`permission:${request}`], now) : [];
  }

  function status(s: Session, value: unknown, now: number): void {
    const status = record(value);
    switch (word(status?.type)) {
      case "busy":
        turnStarted(s, now);
        break;
      case "retry":
        work(s, record(status?.action)?.reason ? "rate_limited" : "retrying", now);
        break;
      case "idle":
        idle(s, now);
        break;
    }
  }

  function ancestors(s: Session): Session[] {
    const chain: Session[] = [];
    let parent = s.parent === undefined ? undefined : sessions.get(s.parent);
    while (parent && parent !== s && !chain.includes(parent)) {
      chain.push(parent);
      parent = parent.parent === undefined ? undefined : sessions.get(parent.parent);
    }
    return chain;
  }

  function descendants(s: Session): Session[] {
    return [...sessions.values()].filter((other) => other !== s && ancestors(other).includes(s));
  }

  /** The strongest of `waits`: its kind, and the start of the earliest
   * pending ask of that kind. */
  function strongest(waits: Wait[]): { detail: WaitKind; since: number } | undefined {
    for (const kind of WAIT_ORDER) {
      const of = waits.filter((wait) => wait.kind === kind);
      if (of.length > 0) return { detail: kind, since: Math.min(...of.map((wait) => wait.since)) };
    }
    return undefined;
  }

  /** What the session shows of itself, before any descendant's wait. */
  function ownView(s: Session): { state: LiveState; detail?: LiveDetail; since: number } | undefined {
    if (s.base?.state === "exited") return s.base;
    const wait = strongest([...s.waits.values()]);
    if (wait) return { state: "waiting", detail: wait.detail, since: wait.since };
    return s.base;
  }

  function active(s: Session): boolean {
    const state = ownView(s)?.state;
    return state === "working" || state === "waiting" || state === "compacting";
  }

  function view(s: Session): View | undefined {
    const family = descendants(s);
    const subagents = family.filter(active).length;
    const cwd = s.cwd ?? directory;
    const own = ownView(s);
    if (own?.state !== "waiting" && own?.state !== "exited") {
      const raised = strongest(family.flatMap((child) => [...child.waits.values()]));
      if (raised) {
        return { state: "waiting", detail: raised.detail, since: raised.since, subagents, via: true, cwd };
      }
    }
    if (!own) return undefined;
    return { state: own.state, detail: own.detail, since: own.since, subagents, via: false, cwd };
  }

  function same(a: View, b: View): boolean {
    return (
      a.state === b.state &&
      a.detail === b.detail &&
      a.since === b.since &&
      a.subagents === b.subagents &&
      a.via === b.via &&
      a.cwd === b.cwd
    );
  }

  function snapshot(s: Session, v: View, name: string, now: number): HookPayload {
    seq += 1;
    const live: LiveSnapshot = {
      instance: options.instance,
      seq,
      observed_ms: now,
      heartbeat_s: heartbeatS,
      ...(options.pid === undefined ? {} : { pid: options.pid }),
      ...(options.pidStart === undefined ? {} : { pid_start: options.pidStart }),
      state: v.state,
      ...(v.detail === undefined ? {} : { detail: v.detail }),
      subagents: v.subagents,
      via_subagent: v.via,
      since_ms: v.since,
    };
    return {
      session_id: s.id,
      ...(v.cwd === undefined ? {} : { cwd: v.cwd }),
      hook_event_name: name,
      live,
    };
  }

  /** The snapshots of the sessions `touched` and their ancestors whose
   * view changed, and for `attributed` an attribution record. */
  function publish(touched: Session[], name: string, now: number, attributed?: Session): HookPayload[] {
    const order: Session[] = [];
    for (const s of touched) {
      for (const each of [s, ...ancestors(s)]) if (!order.includes(each)) order.push(each);
    }
    const out: HookPayload[] = [];
    for (const s of order) {
      let next = view(s);
      if (!next) {
        // Evidence that no longer holds: say so rather than leave the
        // last snapshot standing.
        if (!s.shown || s.shown.state === "unknown") continue;
        next = { state: "unknown", since: now, subagents: 0, via: false, cwd: s.cwd ?? directory };
      }
      if (s.shown && same(s.shown, next)) continue;
      s.shown = next;
      out.push(snapshot(s, next, name, now));
    }
    if (attributed) {
      const cwd = attributed.cwd ?? directory;
      const named: HookPayload = {
        session_id: attributed.id,
        ...(cwd === undefined ? {} : { cwd }),
        hook_event_name: name,
        attribution: true,
      };
      // One write records both when the session's snapshot changed too.
      const index = out.findIndex((payload) => payload.session_id === attributed.id);
      if (index >= 0) out[index] = { ...named, live: out[index].live };
      else out.unshift(named);
    }
    evict();
    return out;
  }

  function evict(): void {
    if (sessions.size <= MAX_SESSIONS) return;
    const quiet = [...sessions.values()]
      .filter((s) => !s.shown || !BEATING.has(s.shown.state))
      .sort((a, b) => a.changed - b.changed);
    for (const s of quiet) {
      if (sessions.size <= MAX_SESSIONS) break;
      sessions.delete(s.id);
    }
  }

  /** A session's last snapshot, then forget it. */
  function remove(s: Session, name: string, now: number): HookPayload[] {
    end(s, now);
    const out = publish([s], name, now);
    sessions.delete(s.id);
    return out;
  }

  function created(s: Session, now: number): void {
    if (!s.base) setBase(s, "ready", undefined, now);
  }

  function v1(event: unknown, now: number): HookPayload[] {
    const e = record(event);
    const type = typeof e?.type === "string" ? e.type : "";
    const p = record(e?.properties) ?? {};
    const attribution = (s: Session) => (ATTRIBUTION_V1.has(type) && s.parent === undefined ? s : undefined);
    switch (type) {
      case "session.created":
      case "session.updated": {
        const info = record(p.info);
        const sessionID = id(info?.id);
        if (!sessionID) return [];
        const s = session(sessionID, now);
        const former = learn(s, info?.parentID, info?.directory);
        if (type === "session.created") created(s, now);
        return publish(former ? [s, former] : [s], type, now, attribution(s));
      }
      case "session.deleted": {
        const s = existing(id(record(p.info)?.id), now);
        return s ? remove(s, type, now) : [];
      }
      case "session.compacted": {
        const s = existing(id(p.sessionID), now);
        if (!s) return [];
        endCompaction(s, now);
        return publish([s], type, now, attribution(s));
      }
      case "permission.replied":
        return publish(reply(id(p.sessionID), p.requestID ?? p.permissionID, p.reply ?? p.response, now), type, now);
      case "question.replied":
      case "question.rejected": {
        const request = id(p.requestID);
        return request ? publish(endWaits(id(p.sessionID), [`question:${request}`], now), type, now) : [];
      }
    }
    const sessionID = id(p.sessionID);
    if (!sessionID) return [];
    switch (type) {
      case "session.status":
      case "session.idle": {
        const s = session(sessionID, now);
        if (type === "session.idle") idle(s, now);
        else status(s, p.status, now);
        return publish([s], type, now, attribution(s));
      }
      case "session.error": {
        const s = session(sessionID, now);
        const ending = endingOfV1Error(record(p.error));
        const state = s.base?.state;
        if (state === undefined || state === "working" || state === "compacting") {
          s.ending = ending;
        } else if (ending === "interrupted") {
          finish(s, "ready", "interrupted", now);
        } else {
          finish(s, "error", ending, now);
        }
        return publish([s], type, now);
      }
      case "permission.asked":
      case "permission.updated":
      case "question.asked": {
        const request = id(p.id);
        if (!request) return [];
        const s = session(sessionID, now);
        startWait(s, type === "question.asked" ? "question" : "permission", request, now);
        return publish([s], type, now);
      }
    }
    return [];
  }

  function v2(item: unknown, now: number): HookPayload[] {
    const outer = record(item);
    const e = record(outer?.details) ?? outer;
    const type = typeof e?.type === "string" ? e.type : "";
    const data = record(e?.data) ?? record(e?.properties) ?? {};
    const location = directoryOf(e?.location);
    // One instance serves one location; an event of another is that
    // instance's to report.
    if (directory && location && !sameDirectory(directory, location)) return [];
    switch (type) {
      case "permission.replied":
        return publish(reply(id(data.sessionID), data.requestID, data.reply, now), type, now);
      case "form.created": {
        const form = record(data.form) ?? data;
        const sessionID = id(form.sessionID);
        const formID = id(form.id);
        // A form outside any session (`global`) has no session to show it.
        if (!sessionID || !formID || sessionID === "global") return [];
        const s = session(sessionID, now);
        const kind = word(record(form.metadata)?.kind) === "question" ? "question" : "input";
        startWait(s, kind, formID, now);
        return publish([s], type, now);
      }
      case "form.replied":
      case "form.cancelled": {
        const form = record(data.form);
        const formID = id(data.id) ?? id(form?.id);
        if (!formID) return [];
        const sessionID = id(data.sessionID) ?? id(form?.sessionID);
        return publish(endWaits(sessionID, [`question:${formID}`, `input:${formID}`], now), type, now);
      }
    }
    const info = record(data.info);
    const sessionID = id(data.sessionID) ?? id(info?.id);
    if (!sessionID) return [];
    switch (type) {
      case "session.created": {
        const s = session(sessionID, now);
        const former = learn(s, data.parentID ?? info?.parentID, directoryOf(data.location));
        created(s, now);
        return publish(former ? [s, former] : [s], type, now);
      }
      case "session.deleted":
      case "session.archived": {
        const s = existing(sessionID, now);
        return s ? remove(s, type, now) : [];
      }
      case "session.execution.started":
        turnStarted(session(sessionID, now), now);
        break;
      case "session.execution.succeeded":
        finish(session(sessionID, now), "ready", "finished", now);
        break;
      case "session.execution.failed":
        finish(session(sessionID, now), "error", failureOfV2Error(record(data.error)), now);
        break;
      case "session.execution.interrupted": {
        // A shutdown keeps the session's claim for the service's next
        // start, but this process is going away: the session ends here.
        const s = session(sessionID, now);
        if (word(data.reason) === "shutdown") end(s, now);
        else finish(s, "ready", "interrupted", now);
        break;
      }
      case "session.retry.scheduled":
        work(session(sessionID, now), rateLimitedV2(record(data.error)) ? "rate_limited" : "retrying", now);
        break;
      case "session.step.started": {
        const s = existing(sessionID, now);
        const detail = s?.base?.state === "working" ? s.base.detail : undefined;
        if (s && (detail === "retrying" || detail === "rate_limited")) work(s, "generating", now);
        break;
      }
      case "session.compaction.started":
        startCompaction(session(sessionID, now), now);
        break;
      case "session.compaction.ended":
      case "session.compaction.failed": {
        const s = existing(sessionID, now);
        if (s) endCompaction(s, now);
        break;
      }
      case "session.status":
        status(session(sessionID, now), data.status, now);
        break;
      case "permission.asked": {
        const request = id(data.id);
        if (!request) return [];
        startWait(session(sessionID, now), "permission", request, now);
        break;
      }
      default:
        return [];
    }
    const s = sessions.get(sessionID);
    return s ? publish([s], type, now) : [];
  }

  function compacting(sessionID: unknown, now: number): HookPayload[] {
    const checked = id(sessionID);
    if (!checked) return [];
    const s = session(checked, now);
    startCompaction(s, now);
    return publish([s], "experimental.session.compacting", now);
  }

  function seed(sessionID: unknown, listed: Listed, now: number): HookPayload[] {
    const checked = id(sessionID);
    if (!checked) return [];
    const asks: Array<{ kind: WaitKind; id: string }> = [];
    const collect = (items: unknown, kindOf: (item: Record<string, unknown>) => WaitKind) => {
      if (!Array.isArray(items)) return;
      for (const raw of items) {
        const item = record(raw);
        const request = id(item?.id);
        // An ask of another session (or of none, a global form) is not
        // this session's wait.
        if (!item || !request || (item.sessionID !== undefined && id(item.sessionID) !== checked)) continue;
        const kind = kindOf(item);
        if (!answered.has(`${kind}:${request}`)) asks.push({ kind, id: request });
      }
    };
    collect(listed.permissions, () => "permission");
    collect(listed.forms, (item) => (word(record(item.metadata)?.kind) === "question" ? "question" : "input"));
    const status = word(listed.status);
    const known = sessions.get(checked);
    const fills = known?.base === undefined && (status === "running" || status === "idle");
    if ((asks.length === 0 && !fills) || known?.base?.state === "exited") return [];
    const s = session(checked, now);
    if (s.base === undefined) {
      if (status === "running") work(s, "generating", now);
      else if (status === "idle") setBase(s, "ready", undefined, now);
    }
    for (const ask of asks) startWait(s, ask.kind, ask.id, now);
    return publish([s], "seed", now);
  }

  function resend(include: (sessionID: string) => boolean, name: string, now: number): HookPayload[] {
    const out: HookPayload[] = [];
    for (const s of sessions.values()) {
      if (!include(s.id)) continue;
      const next = view(s);
      if (!next) continue;
      s.shown = next;
      out.push(snapshot(s, next, name, now));
    }
    return out;
  }

  function lineage(sessionID: string): string[] {
    const chain: string[] = [];
    let parent = sessions.get(sessionID)?.parent;
    while (parent !== undefined && parent !== sessionID && !chain.includes(parent)) {
      chain.push(parent);
      parent = sessions.get(parent)?.parent;
    }
    return chain;
  }

  function heartbeat(now: number): HookPayload[] {
    const out: HookPayload[] = [];
    for (const s of sessions.values()) {
      if (s.shown && BEATING.has(s.shown.state)) out.push(snapshot(s, s.shown, "heartbeat", now));
    }
    return out;
  }

  function dispose(now: number): HookPayload[] {
    const all = [...sessions.values()];
    for (const s of all) {
      const state = s.base?.state;
      if (s.waits.size > 0 || state === "working" || state === "compacting") end(s, now);
    }
    return publish(all, "dispose", now);
  }

  function needsHeartbeat(): boolean {
    for (const s of sessions.values()) if (s.shown && BEATING.has(s.shown.state)) return true;
    return false;
  }

  return { v1, v2, compacting, heartbeat, dispose, needsHeartbeat, seed, resend, lineage };
}

/** Call `owner[name](input)`, keeping `owner` as `this`; reject when it
 * is not a method. */
async function call(owner: Record<string, unknown> | undefined, name: string, input: unknown): Promise<unknown> {
  const method = owner?.[name];
  if (typeof method !== "function") throw new TypeError(`no ${name}`);
  return await (method as (this: unknown, input: unknown) => unknown).call(owner, input);
}

/** Ask opencode, through `api` (a 2.x server plugin's context), for the
 * pending permission requests of the most recent sessions of
 * `directory`, and hand each session that has one to `each`, as
 * {@link LiveReducer.seed} reads it. Permission asks are events opencode
 * does not repeat, so one pending when the plugin starts is seen only
 * this way.
 *
 * It lists at most `limit` sessions (`api.session.list`, page by page
 * through its cursor), then asks each (`api.permission.list`). A context
 * without either call, a call that fails, and an answer it does not
 * recognize are skipped: this only fills in what the events could not
 * say. It reads a session's id and the cursor, and passes the asks on
 * unread. `active` is asked before every call, so a plugin that unloads
 * stops asking. */
export async function readPending(
  api: unknown,
  directory: string | undefined,
  each: (sessionID: string, listed: Listed) => void,
  active: () => boolean = () => true,
  limit: number = MAX_LISTED_SESSIONS,
): Promise<void> {
  const sessionApi = record(record(api)?.session);
  const permissionApi = record(record(api)?.permission);
  const dir = path(directory);
  if (!dir || typeof sessionApi?.list !== "function" || typeof permissionApi?.list !== "function") return;
  const ids: string[] = [];
  let input: Record<string, unknown> = { directory: dir, limit: Math.min(LIST_PAGE, limit) };
  while (ids.length < limit && active()) {
    let page: unknown;
    try {
      page = await call(sessionApi, "list", input);
    } catch {
      return;
    }
    const items = Array.isArray(page) ? page : record(page)?.data;
    if (!Array.isArray(items) || items.length === 0) break;
    for (const item of items) {
      const sessionID = id(record(item)?.id);
      if (sessionID && ids.length < limit && !ids.includes(sessionID)) ids.push(sessionID);
    }
    const next = record(record(page)?.cursor)?.next;
    if (typeof next !== "string" || next.length === 0) break;
    input = { cursor: next };
  }
  for (const sessionID of ids) {
    if (!active()) return;
    let asks: unknown;
    try {
      asks = await call(permissionApi, "list", { sessionID });
    } catch {
      continue;
    }
    if (Array.isArray(asks) && asks.length > 0) each(sessionID, { permissions: asks });
  }
}
