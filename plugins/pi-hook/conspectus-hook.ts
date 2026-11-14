// conspectus-hook: installed by `conspectus hook init pi`; `conspectus hook remove pi` deletes this file.
//
// Pi extension that tells Conspectus which session a Pi process runs and
// what it is doing: working, waiting for you, ready, compacting, or failed
// (ADR 0121, ADR 0206). It runs `conspectus hook write pi` with one flat
// payload, which Conspectus records against the tmux pane and in its
// live-state store.
//
// - `CONSPECTUS_HOOK_BIN` picks the binary; otherwise `conspectus` is
//   found on PATH.
// - `CONSPECTUS_PI_SUBAGENT_TOOLS` names more tools whose calls run
//   subagents, separated by commas, beside the reference `subagent`
//   extension's own; it is read when Pi loads the extension.
// - A missing binary or a failed write is ignored, so Pi is never
//   disturbed. No handler waits on anything.
// - `--no-session` runs and the print, JSON, and RPC modes write nothing:
//   there is no session file, or no pane, to report.
//
// Pi awaits every extension handler in its own loop, so the extension
// keeps the state itself (`createMachine`, below) and starts a writer only
// when what the session shows changes, plus a heartbeat every 60 s while
// it is not ready. It subscribes to no event that streams
// (`message_update`, `tool_execution_update`) and to none that can change
// what Pi does (`tool_call`, `tool_result`, `context`).
//
// | Pi event | Effect |
// | :- | :- |
// | `session_start` | a new or resumed session: ready, or working when Pi is not idle; also names the session for attribution |
// | `session_shutdown` | `quit`, `new`, `resume`, `fork`: the session ends here (exited); `reload`: nothing, the reloaded extension reports afresh |
// | `agent_start` | working (generating); a retry's start leaves retrying |
// | `tool_execution_start` / `_end` | top-level calls only: working (tool); a call of a subagent tool counts as a subagent |
// | `message_end` | an assistant message's stop reason; an error is classified here and its text dropped |
// | `after_provider_response` | a 429 while a run is active: working (rate limited) |
// | `agent_end` | after an error: working (retrying) until the next start or the settle |
// | `agent_settled` | ready (finished), ready (interrupted), or error (its class) |
// | `ui_prompt_start` / `_end` | waiting: `confirm` is a permission; `select`, `input`, `editor` a question |
// | `session_before_compact` | compacting, until `session_compact` or `session_compact_failed` |
// | `session_compact_failed` | not cancelled and not retried: error (failed) |
//
// What the session shows, first match wins (Pi's own status reporter has
// the same order): waiting while a dialog is open; compacting; working
// while a run is active (retrying, rate limited, tool, or generating);
// otherwise how the last run ended. A wait shows its own start; once it
// ends the state under it shows with the start it had. `subagents` counts
// the subagent tool calls in flight.
//
// Choices where Pi's events leave one:
//
// - `ctx.ui.custom()` dialogs are not a wait. Pi fires `ui_prompt_start`
//   for every one, loaders and widgets included, and does not say which
//   block, so marking them would raise false alarms.
// - A plan-approval dialog reads as a question: the dialog's kind cannot
//   say "plan".
// - A change between working sub-states is held for 500 ms and the latest
//   is sent, so a burst of tool calls costs one writer; every other change
//   is sent at once and cancels a held one.
//
// Privacy (ADR 0086 Tier 3): only ids, paths, state names, counts, and
// times leave the extension. It reads an event's ids, its kind or reason
// words, a message's role and stop reason, a response's status, and a
// tool's name, the last only to tell a subagent call. It reads an error
// message only to classify it and never copies it. Prompts, tool
// arguments and results, dialog titles, and message text are never read.

import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";

// Recorded on each hook record so it's clear which extension wrote it;
// `conspectus hook status pi` reads it from an installed copy.
const EXTENSION_VERSION = "3";

/** Seconds between heartbeats while the session is not ready. */
const HEARTBEAT_S = 60;

/** How long a change between working sub-states is held. */
const COALESCE_MS = 500;

/** Tools whose calls run subagents: the reference `subagent` extension's. */
const SUBAGENT_TOOLS: ReadonlySet<string> = new Set(["subagent"]);

/** The variable that names more subagent tools, separated by commas. */
const SUBAGENT_TOOLS_ENV = "CONSPECTUS_PI_SUBAGENT_TOOLS";

/** The longest tool name the variable may name; a longer one is no
 * tool's. */
const MAX_TOOL_NAME_LEN = 256;

/** The Pi events the extension handles. */
const EVENTS = [
  "session_start",
  "session_shutdown",
  "agent_start",
  "agent_end",
  "agent_settled",
  "message_end",
  "tool_execution_start",
  "tool_execution_end",
  "ui_prompt_start",
  "ui_prompt_end",
  "session_before_compact",
  "session_compact",
  "session_compact_failed",
  "after_provider_response",
] as const;

/** The `session_start` reasons Pi documents; any other is named `other`. */
const START_REASONS: ReadonlySet<string> = new Set(["startup", "reload", "new", "resume", "fork"]);

const MAX_ID_LEN = 256;
const MAX_PATH_LEN = 4096;
/** The most of an error message the classifier looks at. */
const MAX_CLASSIFIED = 4096;

export type LiveState = "working" | "waiting" | "ready" | "compacting" | "error" | "exited";

export type LiveDetail =
  | "generating"
  | "tool"
  | "retrying"
  | "rate_limited"
  | "permission"
  | "question"
  | "finished"
  | "interrupted"
  | "auth"
  | "overloaded"
  | "failed"
  | "ended";

/** What an error message says went wrong. Only the state it maps to
 * leaves the extension. */
export type ErrorClass =
  | "rate_limit"
  | "overloaded"
  | "auth"
  | "context_overflow"
  | "network"
  | "provider"
  | "unknown";

/** The session's whole state, as `conspectus hook write pi` reads it from
 * a payload's `live` object. */
export type LiveSnapshot = {
  /** This extension load: a fresh id each time Pi loads it. */
  instance: string;
  /** Counts up with every snapshot the load sends. */
  seq: number;
  /** When the snapshot was taken, Unix epoch milliseconds. */
  observed_ms: number;
  heartbeat_s: number;
  /** The Pi process. */
  pid?: number;
  /** Its start time, field 22 of `/proc/<pid>/stat`. */
  pid_start?: number;
  state: LiveState;
  detail?: LiveDetail;
  subagents: number;
  /** When the state began. */
  since_ms: number;
};

/** The flat payload `conspectus hook write pi` reads on stdin. A
 * `session_start:<reason>` payload also names the session for
 * attribution; a `live:<event>` payload only updates the live state. */
export type HookPayload = {
  session_id: string;
  cwd?: string;
  transcript_path?: string;
  hook_event_name: string;
  live: LiveSnapshot;
};

/** What `session_start` learns from Pi's context. */
export type SessionContext = {
  mode?: unknown;
  idle?: unknown;
  session_id?: unknown;
  session_file?: unknown;
  cwd?: unknown;
};

/** What a step sends now, and when the driver must call back. */
export type Output = {
  /** Payloads to send, in order. */
  emit: HookPayload[];
  /** When to call {@link Machine.flush}: a held change is due. */
  flush_at?: number;
  /** When to call {@link Machine.heartbeat}. */
  heartbeat_at?: number;
};

export type MachineOptions = {
  instance: string;
  pid?: number;
  pidStart?: number;
  heartbeatS?: number;
  coalesceMs?: number;
  subagentTools?: ReadonlySet<string>;
};

export type Machine = {
  /** One Pi event. `session` is read only for `session_start`. */
  on(name: string, event: unknown, now: number, session?: SessionContext): Output;
  /** The coalescing window ended. */
  flush(now: number): Output;
  /** The heartbeat is due. */
  heartbeat(now: number): Output;
};

type View = { state: LiveState; detail?: LiveDetail; since: number; subagents: number };

type Outcome = "completed" | "error" | "aborted" | "length" | "deferred";

type Session = {
  id: string;
  cwd?: string;
  file?: string;
  /** A run is active: from `agent_start` to `agent_settled`. */
  run: boolean;
  runSince: number;
  /** When compaction began, while it runs. */
  compacting?: number;
  /** The open dialog. */
  prompt?: { detail: "permission" | "question"; since: number };
  /** Top-level tool calls in flight, by call id: whether each is a
   * subagent's. */
  tools: Map<string, boolean>;
  /** How the latest assistant message of the run ended. */
  outcome?: Outcome;
  errorClass?: ErrorClass;
  /** The latest error status a provider answered this request with. */
  errorStatus?: number;
  /** Between a failed attempt's `agent_end` and the next start. */
  retrying: boolean;
  /** A provider answered 429 and no later request succeeded. */
  rateLimited: boolean;
  /** Compaction failed during this run. */
  compactionFailed: boolean;
  /** What the session shows when no run, dialog, or compaction is on. */
  rest: { state: "ready" | "error"; detail?: LiveDetail; since: number };
  /** The last snapshot sent. */
  shown?: View;
  sentAt: number;
  /** When a held change began, while one is held. */
  heldSince?: number;
  heldName: string;
};

function record(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : undefined;
}

/** `value` when it looks like an id: a short string with no whitespace or
 * control character, as the store requires. */
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

/** A word to compare, lowercased; it is never copied anywhere. */
function word(value: unknown): string | undefined {
  return typeof value === "string" ? value.toLowerCase() : undefined;
}

function number(value: unknown): number | undefined {
  return typeof value === "number" && Number.isFinite(value) ? value : undefined;
}

/** The class an HTTP status says, when it says one. */
function classOfStatus(status: number | undefined): ErrorClass | undefined {
  if (status === undefined) return undefined;
  if (status === 429) return "rate_limit";
  if (status === 401 || status === 403) return "auth";
  if (status === 503 || status === 529) return "overloaded";
  if (status >= 500 && status <= 599) return "provider";
  return undefined;
}

/** What an error message says went wrong, from the words providers and
 * Node use. The message is read here and nowhere else. */
function classifyError(message: unknown): ErrorClass {
  if (typeof message !== "string") return "unknown";
  const text = message.slice(0, MAX_CLASSIFIED).toLowerCase();
  const has = (...words: string[]) => words.some((each) => text.includes(each));
  if (
    /\b429\b/.test(text) ||
    has("rate limit", "rate_limit", "ratelimit", "too many requests", "quota", "usage limit", "resource_exhausted", "resource exhausted")
  ) {
    return "rate_limit";
  }
  if (/\b(503|529)\b/.test(text) || has("overloaded", "service unavailable", "capacity")) return "overloaded";
  if (
    /\b(401|403)\b/.test(text) ||
    has("unauthorized", "unauthorised", "forbidden", "authentication", "api key", "api_key", "x-api-key", "invalid token", "expired token", "token expired", "oauth", "credential", "login")
  ) {
    return "auth";
  }
  if (
    has("context length", "context_length", "context window", "maximum context", "prompt is too long", "input is too long", "too many tokens", "context overflow")
  ) {
    return "context_overflow";
  }
  if (
    has("econnreset", "econnrefused", "etimedout", "enotfound", "eai_again", "socket hang up", "fetch failed", "network", "timed out", "timeout", "connection")
  ) {
    return "network";
  }
  if (/\b5\d\d\b/.test(text) || has("internal server error", "bad gateway", "server error")) return "provider";
  return "unknown";
}

/** The `error` detail a class shows as, in Conspectus's vocabulary. */
function failureDetail(errorClass: ErrorClass | undefined): LiveDetail {
  if (errorClass === "rate_limit") return "rate_limited";
  if (errorClass === "auth") return "auth";
  if (errorClass === "overloaded") return "overloaded";
  return "failed";
}

function outcomeOf(stopReason: string | undefined): Outcome {
  switch (stopReason) {
    case "error":
      return "error";
    case "aborted":
      return "aborted";
    case "length":
      return "length";
    case "deferred":
      return "deferred";
    default:
      return "completed";
  }
}

/** The `waiting` detail a dialog kind shows as; none for `custom` and
 * kinds Pi may add. */
function promptDetail(kind: string | undefined): "permission" | "question" | undefined {
  if (kind === "confirm") return "permission";
  if (kind === "select" || kind === "input" || kind === "editor") return "question";
  return undefined;
}

/** States that age without evidence, so their snapshot is resent. */
function beating(view: View | undefined): boolean {
  return view !== undefined && view.state !== "ready" && view.state !== "exited";
}

function same(a: View, b: View): boolean {
  return a.state === b.state && a.detail === b.detail && a.since === b.since && a.subagents === b.subagents;
}

/** The start time of a process, field 22 of a `/proc/<pid>/stat` line,
 * counted past the command name in parentheses (which may hold spaces or
 * parentheses itself), as Conspectus reads it. */
function parseProcStartTime(stat: string): number | undefined {
  const close = stat.lastIndexOf(")");
  if (close < 0) return undefined;
  const fields = stat.slice(close + 1).trim().split(/\s+/);
  const start = fields[19];
  return start !== undefined && /^\d+$/.test(start) ? Number(start) : undefined;
}

/** A fresh instance id: the time and 64 random bits, which is unique
 * enough to tell one extension load from another. */
function newInstanceId(now: number = Date.now(), random: () => number = Math.random): string {
  let bits = "";
  for (let i = 0; i < 4; i++) {
    bits += Math.floor(random() * 0x10000)
      .toString(16)
      .padStart(4, "0");
  }
  return `${Math.max(0, Math.floor(now)).toString(36)}-${bits}`;
}

/** The live state of the session one extension load reports. Pure: it
 * keeps no timer and starts nothing; the driver sends what it returns and
 * calls back when it asks. */
function createMachine(options: MachineOptions): Machine {
  const heartbeatS = options.heartbeatS ?? HEARTBEAT_S;
  const coalesceMs = options.coalesceMs ?? COALESCE_MS;
  const subagentTools = options.subagentTools ?? SUBAGENT_TOOLS;
  let seq = 0;
  let session: Session | undefined;

  function fresh(sessionID: string, cwd: string | undefined, file: string | undefined, now: number): Session {
    return {
      id: sessionID,
      ...(cwd === undefined ? {} : { cwd }),
      ...(file === undefined ? {} : { file }),
      run: false,
      runSince: now,
      tools: new Map(),
      retrying: false,
      rateLimited: false,
      compactionFailed: false,
      rest: { state: "ready", since: now },
      sentAt: now,
      heldName: "",
    };
  }

  function view(s: Session): View {
    let subagents = 0;
    for (const subagent of s.tools.values()) if (subagent) subagents += 1;
    if (s.prompt) return { state: "waiting", detail: s.prompt.detail, since: s.prompt.since, subagents };
    if (s.compacting !== undefined) return { state: "compacting", since: s.compacting, subagents };
    if (s.run) {
      let detail: LiveDetail = "generating";
      if (s.retrying) detail = s.rateLimited || s.errorClass === "rate_limit" ? "rate_limited" : "retrying";
      else if (s.rateLimited) detail = "rate_limited";
      else if (s.tools.size > 0) detail = "tool";
      return { state: "working", detail, since: s.runSince, subagents };
    }
    return { ...s.rest, subagents };
  }

  function snapshot(s: Session, v: View, name: string, now: number): HookPayload {
    seq += 1;
    return {
      session_id: s.id,
      ...(s.cwd === undefined ? {} : { cwd: s.cwd }),
      ...(s.file === undefined ? {} : { transcript_path: s.file }),
      hook_event_name: name,
      live: {
        instance: options.instance,
        seq,
        observed_ms: now,
        heartbeat_s: heartbeatS,
        ...(options.pid === undefined ? {} : { pid: options.pid }),
        ...(options.pidStart === undefined ? {} : { pid_start: options.pidStart }),
        state: v.state,
        ...(v.detail === undefined ? {} : { detail: v.detail }),
        subagents: v.subagents,
        since_ms: v.since,
      },
    };
  }

  function output(emit: HookPayload[]): Output {
    const s = session;
    const out: Output = { emit };
    if (s?.heldSince !== undefined) out.flush_at = s.heldSince + coalesceMs;
    if (s && beating(s.shown)) out.heartbeat_at = s.sentAt + heartbeatS * 1000;
    return out;
  }

  function send(s: Session, v: View, name: string, now: number): HookPayload {
    s.heldSince = undefined;
    s.shown = v;
    s.sentAt = now;
    return snapshot(s, v, name, now);
  }

  /** Send what the session shows when it changed: at once, or after the
   * coalescing window for a change between working sub-states. */
  function publish(s: Session, name: string, now: number): Output {
    const next = view(s);
    if (s.shown && same(s.shown, next)) {
      s.heldSince = undefined;
      return output([]);
    }
    if (s.shown?.state === "working" && next.state === "working") {
      if (s.heldSince === undefined) {
        s.heldSince = now;
        s.heldName = name;
      }
      return output([]);
    }
    return output([send(s, next, name, now)]);
  }

  /** The run is active from now, if it was not. */
  function runs(s: Session, now: number): void {
    if (s.run) return;
    s.run = true;
    s.runSince = now;
    s.outcome = undefined;
    s.errorClass = undefined;
    s.compactionFailed = false;
  }

  function start(event: Record<string, unknown>, now: number, context: SessionContext | undefined): Output {
    const emit: HookPayload[] = [];
    const sessionID = id(context?.session_id);
    const previous = session;
    session = undefined;
    // A session left without a shutdown ends here.
    if (previous && previous.id !== sessionID) {
      emit.push(snapshot(previous, { state: "exited", detail: "ended", since: now, subagents: 0 }, "live:session_start", now));
    }
    const file = path(context?.session_file);
    if (context?.mode !== "tui" || !sessionID || !file) return { emit };
    const s = fresh(sessionID, path(context?.cwd), file, now);
    session = s;
    if (context?.idle === false) runs(s, now);
    const reason = word(event.reason);
    const named = reason !== undefined && START_REASONS.has(reason) ? reason : "other";
    emit.push(send(s, view(s), `session_start:${named}`, now));
    return output(emit);
  }

  function shutdown(event: Record<string, unknown>, now: number): Output {
    const s = session;
    session = undefined;
    // A reload loads the extension again, and the new load reports the
    // same session afresh.
    if (!s || word(event.reason) === "reload") return { emit: [] };
    return { emit: [snapshot(s, { state: "exited", detail: "ended", since: now, subagents: 0 }, "live:session_shutdown", now)] };
  }

  function on(name: string, raw: unknown, now: number, context?: SessionContext): Output {
    const event = record(raw) ?? {};
    if (name === "session_start") return start(event, now, context);
    if (name === "session_shutdown") return shutdown(event, now);
    const s = session;
    if (!s) return { emit: [] };
    const live = `live:${name}`;
    switch (name) {
      case "agent_start":
        runs(s, now);
        s.retrying = false;
        s.rateLimited = false;
        s.errorStatus = undefined;
        return publish(s, live, now);
      case "agent_end":
        if (s.run && s.outcome === "error") s.retrying = true;
        return publish(s, live, now);
      case "agent_settled": {
        if (!s.run) return output([]);
        const aborted = event.aborted === true || s.outcome === "aborted";
        if (aborted) s.rest = { state: "ready", detail: "interrupted", since: now };
        else if (s.outcome === "error") s.rest = { state: "error", detail: failureDetail(s.errorClass), since: now };
        else if (s.compactionFailed) s.rest = { state: "error", detail: "failed", since: now };
        else s.rest = { state: "ready", detail: "finished", since: now };
        s.run = false;
        s.tools.clear();
        s.retrying = false;
        s.rateLimited = false;
        s.errorStatus = undefined;
        return publish(s, live, now);
      }
      case "message_end": {
        const message = record(event.message);
        if (word(message?.role) !== "assistant") return output([]);
        s.outcome = outcomeOf(word(message?.stopReason));
        s.errorClass =
          s.outcome === "error" ? (classOfStatus(s.errorStatus) ?? classifyError(message?.errorMessage)) : undefined;
        return output([]);
      }
      case "tool_execution_start":
      case "tool_execution_end": {
        // A call made by another tool is that tool's work.
        if (event.parentToolCallId !== undefined && event.parentToolCallId !== null) return output([]);
        const call = id(event.toolCallId);
        if (!call) return output([]);
        if (name === "tool_execution_start") {
          runs(s, now);
          s.tools.set(call, typeof event.toolName === "string" && subagentTools.has(event.toolName));
        } else if (!s.tools.delete(call)) {
          return output([]);
        }
        return publish(s, live, now);
      }
      case "after_provider_response": {
        const status = number(event.status);
        if (!s.run || status === undefined) return output([]);
        if (status >= 400) {
          s.errorStatus = status;
          if (status === 429) s.rateLimited = true;
        } else {
          s.errorStatus = undefined;
          s.rateLimited = false;
        }
        return publish(s, live, now);
      }
      case "ui_prompt_start": {
        const detail = promptDetail(word(event.kind));
        if (!detail || s.prompt) return output([]);
        s.prompt = { detail, since: now };
        return publish(s, live, now);
      }
      case "ui_prompt_end":
        if (!promptDetail(word(event.kind)) || !s.prompt) return output([]);
        s.prompt = undefined;
        return publish(s, live, now);
      case "session_before_compact":
        if (s.compacting === undefined) s.compacting = now;
        return publish(s, live, now);
      case "session_compact":
      case "session_compact_failed": {
        s.compacting = undefined;
        if (event.willRetry === true) {
          runs(s, now);
        } else if (name === "session_compact_failed" && event.aborted !== true) {
          if (s.run) s.compactionFailed = true;
          else s.rest = { state: "error", detail: "failed", since: now };
        }
        return publish(s, live, now);
      }
    }
    return output([]);
  }

  function flush(now: number): Output {
    const s = session;
    if (!s || s.heldSince === undefined) return output([]);
    s.heldSince = undefined;
    const next = view(s);
    if (s.shown && same(s.shown, next)) return output([]);
    return output([send(s, next, s.heldName, now)]);
  }

  function heartbeat(now: number): Output {
    const s = session;
    if (!s || !beating(s.shown)) return output([]);
    return output([send(s, view(s), "live:heartbeat", now)]);
  }

  return { on, flush, heartbeat };
}

/** The tools whose calls count as subagents: {@link SUBAGENT_TOOLS} and
 * the names `value` (the variable's value) lists, separated by commas,
 * each trimmed. An empty name, a long one, or one holding a control
 * character is skipped. A name is only ever compared with a call's tool
 * name, never sent. */
function subagentTools(value: unknown): ReadonlySet<string> {
  const tools = new Set(SUBAGENT_TOOLS);
  if (typeof value !== "string") return tools;
  for (const part of value.split(",")) {
    const name = part.trim();
    if (name.length > 0 && name.length <= MAX_TOOL_NAME_LEN && !/\p{Cc}/u.test(name)) tools.add(name);
  }
  return tools;
}

/** For the extension's own tests (`test/conspectus-hook.test.ts`) only.
 * Pi loads the default export; nothing here is part of what Pi or
 * Conspectus relies on, and it may change with any version. */
export const testOnly = {
  COALESCE_MS,
  EVENTS,
  EXTENSION_VERSION,
  HEARTBEAT_S,
  SUBAGENT_TOOLS,
  SUBAGENT_TOOLS_ENV,
  classifyError,
  createMachine,
  newInstanceId,
  parseProcStartTime,
  subagentTools,
};

/** This process's start time as `/proc/self/stat` gives it, the value
 * Conspectus compares to tell a reused pid; none off Linux. */
function ownStartTime(): number | undefined {
  try {
    return parseProcStartTime(readFileSync("/proc/self/stat", "utf8"));
  } catch {
    return undefined;
  }
}

/** What `session_start` needs from Pi's context, read once, so no timer
 * ever touches a context Pi may since have replaced. */
function sessionContext(ctx: unknown): SessionContext {
  try {
    const c = ctx as {
      mode?: unknown;
      cwd?: unknown;
      isIdle?: () => unknown;
      sessionManager?: {
        getSessionId?: () => unknown;
        getSessionFile?: () => unknown;
        getCwd?: () => unknown;
      };
    };
    return {
      mode: c.mode,
      idle: typeof c.isIdle === "function" ? c.isIdle() : undefined,
      session_id: c.sessionManager?.getSessionId?.(),
      session_file: c.sessionManager?.getSessionFile?.(),
      cwd: c.sessionManager?.getCwd?.() ?? c.cwd,
    };
  } catch {
    return {};
  }
}

/** Start `conspectus hook write pi` with `payload` on its stdin and leave
 * it. The payload is written before this returns, so it lands even when
 * Pi exits right after (a quit); the writer runs in its own process
 * group, so the pane's hangup does not stop it, and it never keeps Pi
 * running. */
function write(payload: HookPayload): void {
  try {
    const child = spawn(
      process.env.CONSPECTUS_HOOK_BIN || "conspectus",
      ["hook", "write", "pi"],
      {
        stdio: ["pipe", "ignore", "ignore"],
        detached: true,
        env: { ...process.env, CONSPECTUS_PI_HOOK_VERSION: EXTENSION_VERSION },
      },
    );
    child.on("error", () => {});
    child.stdin.on("error", () => {});
    child.stdin.end(JSON.stringify(payload));
    child.unref();
  } catch {
    // Ignored: Pi keeps running without live state.
  }
}

type Handler = (event: unknown, ctx: unknown) => void;

export default function (pi: ExtensionAPI) {
  const machine = createMachine({
    instance: newInstanceId(),
    pid: process.pid,
    pidStart: ownStartTime(),
    subagentTools: subagentTools(process.env[SUBAGENT_TOOLS_ENV]),
  });
  const timers: { flush?: ReturnType<typeof setTimeout>; heartbeat?: ReturnType<typeof setTimeout> } = {};
  const due: { flush?: number; heartbeat?: number } = {};

  const schedule = (kind: "flush" | "heartbeat", at: number | undefined) => {
    if (due[kind] === at) return;
    clearTimeout(timers[kind]);
    timers[kind] = undefined;
    due[kind] = at;
    if (at === undefined) return;
    const timer = setTimeout(
      () => {
        timers[kind] = undefined;
        due[kind] = undefined;
        run(() => (kind === "flush" ? machine.flush(Date.now()) : machine.heartbeat(Date.now())));
      },
      Math.max(0, at - Date.now()),
    );
    // A pending timer never keeps Pi running.
    timer.unref?.();
    timers[kind] = timer;
  };

  const run = (step: () => Output) => {
    try {
      const output = step();
      for (const payload of output.emit) write(payload);
      schedule("flush", output.flush_at);
      schedule("heartbeat", output.heartbeat_at);
    } catch {
      // A bug here must not reach Pi; the next event repairs the state.
    }
  };

  for (const name of EVENTS) {
    const handler: Handler = (event, ctx) => {
      run(() => machine.on(name, event, Date.now(), name === "session_start" ? sessionContext(ctx) : undefined));
    };
    (pi as unknown as { on(name: string, handler: Handler): void }).on(name, handler);
  }
}
