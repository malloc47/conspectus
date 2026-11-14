// The live-state reducer (`src/live.ts`) against the shared event
// scripts in `tests/fixtures/hooks/opencode-live/`. Each script is an
// opencode major, a header with the plugin instance's options, and steps:
// an input (an `event`, the 1.x `compacting` hook, a `heartbeat`,
// `dispose`, or what opencode lists of sessions to `seed`), the payloads
// the reducer must `emit` after it, in order,
// and what each session it names is `shown` as. The Rust tests feed the
// same payloads through `conspectus hook write opencode`'s parser and
// the live-state store, and check `shown` there too.
//
// Run with Node 22.6 or later: `npm test` in `plugins/opencode-hook`, or
// `just opencode-plugin-test`. With `OPENCODE_LIVE_UPDATE=1` the run
// rewrites each step's `emit` from the reducer instead of comparing it;
// review the diff before keeping it.

import { test } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import {
  MAX_LISTED_SESSIONS,
  MAX_SESSIONS,
  V1_EVENTS,
  V2_EVENTS,
  createLiveState,
  newInstanceId,
  parseProcStartTime,
  readPending,
  type HookPayload,
  type Listed,
  type LiveReducer,
  type LiveSnapshot,
} from "../src/live.ts";

const FIXTURES = fileURLToPath(new URL("../../../tests/fixtures/hooks/opencode-live/", import.meta.url));
const UPDATE = process.env.OPENCODE_LIVE_UPDATE === "1";

type Header = {
  synthetic: boolean;
  major: 1 | 2;
  scenario: string;
  instance: string;
  pid?: number;
  pid_start?: number;
  directory?: string;
};

type Shown = {
  state: string;
  detail?: string;
  since_ms?: number;
  subagents?: number;
  via_subagent?: boolean;
  confidence?: string;
  liveness?: string;
};

type Step = {
  at_ms: number;
  note?: string;
  event?: unknown;
  compacting?: string;
  heartbeat?: boolean;
  dispose?: boolean;
  seed?: Record<string, Listed>;
  emit?: HookPayload[];
  shown?: Record<string, Shown>;
};

type Script = { name: string; header: Header; steps: Step[] };

function scripts(): Script[] {
  return readdirSync(FIXTURES)
    .filter((file) => file.endsWith(".jsonl"))
    .sort()
    .map((file) => {
      const [header, ...steps] = readFileSync(FIXTURES + file, "utf8")
        .split("\n")
        .filter((line) => line.trim() !== "")
        .map((line) => JSON.parse(line));
      return { name: file.replace(/\.jsonl$/, ""), header, steps };
    });
}

function reducer(header: Header): LiveReducer {
  return createLiveState({
    instance: header.instance,
    pid: header.pid,
    pidStart: header.pid_start,
    directory: header.directory,
  });
}

/** The payloads `step`'s input makes the reducer emit; `undefined` for a
 * step that only checks. */
function run(live: LiveReducer, major: 1 | 2, step: Step): HookPayload[] | undefined {
  if (step.event !== undefined) {
    return major === 1 ? live.v1(step.event, step.at_ms) : live.v2(step.event, step.at_ms);
  }
  if (step.compacting !== undefined) return live.compacting(step.compacting, step.at_ms);
  if (step.heartbeat) return live.heartbeat(step.at_ms);
  if (step.dispose) return live.dispose(step.at_ms);
  if (step.seed !== undefined) {
    return Object.entries(step.seed).flatMap(([session, listed]) => live.seed(session, listed, step.at_ms));
  }
  return undefined;
}

/** Whether a check is one only the store can make: a reader's confidence
 * or a process's liveness. */
function storeOnly(expect: Shown): boolean {
  return (expect.confidence ?? "fresh") !== "fresh" || (expect.liveness ?? "alive") !== "alive";
}

function checkShown(latest: Map<string, LiveSnapshot>, step: Step, place: string): void {
  for (const [session, expect] of Object.entries(step.shown ?? {})) {
    if (storeOnly(expect)) continue;
    const live = latest.get(session);
    assert.ok(live, `${place}: nothing sent for ${session}`);
    assert.equal(live.state, expect.state, `${place}: ${session} state`);
    assert.equal(live.detail, expect.detail, `${place}: ${session} detail`);
    assert.equal(live.subagents, expect.subagents ?? 0, `${place}: ${session} subagents`);
    assert.equal(live.via_subagent, expect.via_subagent ?? false, `${place}: ${session} via_subagent`);
    if (expect.since_ms !== undefined) {
      assert.equal(live.since_ms, expect.since_ms, `${place}: ${session} since_ms`);
    }
  }
}

/** The script with each step's keys in reading order: when, why, the
 * input, what it emits, what that shows. */
function rewrite(script: Script): void {
  const order = ["at_ms", "note", "event", "compacting", "heartbeat", "dispose", "seed", "emit", "shown"];
  const ordered = script.steps.map((step) => {
    const entries = Object.entries(step);
    const rank = (key: string) => (order.includes(key) ? order.indexOf(key) : order.length);
    return Object.fromEntries(entries.sort(([a], [b]) => rank(a) - rank(b)));
  });
  const lines = [script.header, ...ordered].map((line) => JSON.stringify(line));
  writeFileSync(`${FIXTURES}${script.name}.jsonl`, lines.join("\n") + "\n");
}

for (const script of scripts()) {
  test(`${script.name}: ${script.header.scenario}`, () => {
    assert.equal(script.header.synthetic, true, "every script is labeled synthetic");
    const live = reducer(script.header);
    const latest = new Map<string, LiveSnapshot>();
    let seq = 0;
    script.steps.forEach((step, index) => {
      const place = `${script.name} step ${index + 1}${step.note ? ` (${step.note})` : ""}`;
      const emitted = run(live, script.header.major, step);
      if (emitted === undefined) {
        assert.equal(step.emit, undefined, `${place}: a check-only step emits nothing`);
        return;
      }
      if (UPDATE) step.emit = emitted;
      assert.deepEqual(emitted, step.emit, place);
      for (const payload of emitted) {
        assert.ok(!JSON.stringify(payload).includes("SENTINEL"), `${place}: payload text left the reducer`);
        if (payload.live) {
          seq += 1;
          assert.equal(payload.live.seq, seq, `${place}: seq counts up by one`);
          latest.set(payload.session_id, payload.live);
        }
      }
      checkShown(latest, step, place);
    });
    if (UPDATE) rewrite(script);
  });
}

test("every subscribed event has a script of its major", () => {
  const seen = { 1: new Set<string>(), 2: new Set<string>() };
  for (const script of scripts()) {
    for (const step of script.steps) {
      const event = step.event as { type?: string; details?: { type?: string } } | undefined;
      const type = event?.type ?? event?.details?.type;
      if (type) seen[script.header.major].add(type);
    }
  }
  for (const type of V1_EVENTS) assert.ok(seen[1].has(type), `1.x ${type}`);
  for (const type of V2_EVENTS) assert.ok(seen[2].has(type), `2.x ${type}`);
});

test("the scripts plant a sentinel in every payload-bearing field", () => {
  // Every key that holds a sentinel somewhere under it.
  const fields = new Set<string>();
  const walk = (value: unknown, keys: string[]) => {
    if (typeof value === "string" && value.includes("SENTINEL")) keys.forEach((key) => fields.add(key));
    else if (Array.isArray(value)) value.forEach((item) => walk(item, keys));
    else if (typeof value === "object" && value !== null) {
      for (const [key, item] of Object.entries(value)) walk(item, [...keys, key]);
    }
  };
  for (const script of scripts()) for (const step of script.steps) walk([step.event, step.seed], []);
  for (const field of [
    "title",
    "message",
    "patterns",
    "pattern",
    "always",
    "metadata",
    "command",
    "resources",
    "save",
    "questions",
    "question",
    "answers",
    "fields",
    "answer",
    "responseBody",
    "response",
    "link",
  ]) {
    assert.ok(fields.has(field), field);
  }
});

test("ids that do not look like ids are refused", () => {
  const live = createLiveState({ instance: "test-instance", directory: "/work" });
  const busy = (sessionID: unknown) => ({ type: "session.status", properties: { sessionID, status: { type: "busy" } } });
  assert.deepEqual(live.v1(busy("SENTINEL text with spaces"), 1), []);
  assert.deepEqual(live.v1(busy(""), 1), []);
  assert.deepEqual(live.v1(busy("x".repeat(257)), 1), []);
  assert.deepEqual(live.v1(busy(42), 1), []);
  assert.deepEqual(live.v1(busy("ses_ok\u0007bell"), 1), []);
  assert.equal(live.v1(busy("ses_8charsok"), 1).length, 1);
  const ask = { type: "permission.asked", properties: { id: "per with space", sessionID: "ses_8charsok" } };
  assert.deepEqual(live.v1(ask, 2), []);
});

test("anything that is not an event it knows changes nothing", () => {
  const live = createLiveState({ instance: "test-instance" });
  for (const event of [null, undefined, 7, "session.status", [], {}, { type: 3 }, { type: "message.part.updated", properties: { sessionID: "ses_8charsok", delta: "SENTINEL" } }]) {
    assert.deepEqual(live.v1(event, 1), []);
    assert.deepEqual(live.v2(event, 1), []);
  }
  assert.deepEqual(live.compacting(undefined, 1), []);
  assert.deepEqual(live.heartbeat(1), []);
  assert.deepEqual(live.dispose(1), []);
  assert.equal(live.needsHeartbeat(), false);
});

test("a session with no evidence shows nothing, and a raised wait that ends says unknown", () => {
  const live = createLiveState({ instance: "test-instance", directory: "/work" });
  // A parent heard of only through session info: attribution, no state.
  const updated = live.v1({ type: "session.updated", properties: { info: { id: "ses_parent001" } } }, 1);
  assert.deepEqual(updated, [
    { session_id: "ses_parent001", cwd: "/work", hook_event_name: "session.updated", attribution: true },
  ]);
  live.v1({ type: "session.created", properties: { info: { id: "ses_child0001", parentID: "ses_parent001" } } }, 1);
  const raised = live.v1({ type: "permission.asked", properties: { id: "per_1", sessionID: "ses_child0001" } }, 2);
  assert.deepEqual(
    raised.map((payload) => [payload.session_id, payload.live?.state, payload.live?.via_subagent]),
    [
      ["ses_child0001", "waiting", false],
      ["ses_parent001", "waiting", true],
    ],
  );
  const ended = live.v1(
    { type: "permission.replied", properties: { requestID: "per_1", sessionID: "ses_child0001", reply: "once" } },
    3,
  );
  assert.deepEqual(
    ended.map((payload) => [payload.session_id, payload.live?.state, payload.live?.detail]),
    [
      ["ses_child0001", "working", "generating"],
      ["ses_parent001", "unknown", undefined],
    ],
  );
});

test("an instance forgets the least recently changed idle sessions past its limit", () => {
  const live = createLiveState({ instance: "test-instance", directory: "/work" });
  const created = (n: number) => ({ type: "session.created", properties: { info: { id: `ses_${String(n).padStart(8, "0")}` } } });
  for (let n = 0; n <= MAX_SESSIONS; n++) live.v1(created(n), n + 1);
  // The first was forgotten: a status for it starts from no evidence, so
  // its idle shows a finished turn where a remembered ready session
  // would show nothing new.
  const idle = (n: number) => ({ type: "session.status", properties: { sessionID: `ses_${String(n).padStart(8, "0")}`, status: { type: "idle" } } });
  assert.equal(live.v1(idle(0), 10_000).find((payload) => payload.live)?.live?.detail, "finished");
  assert.equal(live.v1(idle(MAX_SESSIONS), 10_001).find((payload) => payload.live), undefined);
});

test("the start time is field twenty-two counted past the command name", () => {
  const stat = (command: string, start: number) =>
    `4242 (${command}) S 1 4242 4242 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 1 0 ${start} 1000 10`;
  assert.equal(parseProcStartTime(stat("opencode", 987654)), 987654);
  assert.equal(parseProcStartTime(stat("a (weird) name", 31337)), 31337);
  assert.equal(parseProcStartTime("4242 (short) S 1"), undefined);
  assert.equal(parseProcStartTime("no parentheses"), undefined);
  try {
    const own = readFileSync("/proc/self/stat", "utf8");
    assert.ok(Number.isInteger(parseProcStartTime(own)), own);
  } catch (err) {
    if ((err as NodeJS.ErrnoException).code !== "ENOENT") throw err;
  }
});

test("instance ids look like ids and differ", () => {
  const ids = new Set(Array.from({ length: 100 }, () => newInstanceId()));
  assert.equal(ids.size, 100);
  for (const value of ids) assert.match(value, /^[0-9a-z]+-[0-9a-f]{16}$/);
  assert.equal(newInstanceId(0, () => 0), "0-0000000000000000");
});

/** `value` behind a proxy that records every key read from it, and from
 * the objects it holds, as `path.key`. */
function watched<T>(value: T, reads: Set<string>, at = ""): T {
  if (typeof value !== "object" || value === null) return value;
  return new Proxy(value as object, {
    get(target, key, receiver) {
      const item = Reflect.get(target, key, receiver);
      if (typeof key !== "string" || (Array.isArray(target) && (key === "length" || /^\d+$/.test(key)))) {
        return Array.isArray(target) ? watched(item, reads, at) : item;
      }
      if (Array.isArray(target)) return item;
      const where = at ? `${at}.${key}` : key;
      reads.add(where);
      return watched(item, reads, where);
    },
  }) as T;
}

const ask = (id: string, sessionID: string) => ({
  id,
  sessionID,
  action: "SENTINEL action",
  resources: ["SENTINEL resource"],
  save: ["SENTINEL save"],
  metadata: { command: "SENTINEL metadata" },
  source: { type: "tool", messageID: "msg_1", id: "call_1" },
  message: "SENTINEL message",
});

const form = (id: string, sessionID: string, kind?: string) => ({
  id,
  sessionID,
  title: "SENTINEL title",
  fields: [{ name: "SENTINEL field" }],
  metadata: kind === undefined ? undefined : { kind, tool: { callID: "call_2" }, note: "SENTINEL note" },
});

test("a seed reads only an ask's id and session, a form's kind, and the status word", () => {
  const live = createLiveState({ instance: "test-instance", directory: "/work" });
  const reads = new Set<string>();
  const out = live.seed(
    "ses_8charsok",
    watched(
      {
        status: "running",
        permissions: [ask("per_1", "ses_8charsok"), ask("per_2", "ses_another1"), ask("per with space", "ses_8charsok")],
        forms: [form("frm_1", "ses_8charsok", "question"), form("frm_2", "global"), form("frm_3", "ses_8charsok")],
      },
      reads,
    ),
    10,
  );
  assert.deepEqual([...reads].sort(), [
    "forms",
    "forms.id",
    "forms.metadata",
    "forms.metadata.kind",
    "forms.sessionID",
    "permissions",
    "permissions.id",
    "permissions.sessionID",
    "status",
  ]);
  assert.equal(out.length, 1);
  assert.ok(!JSON.stringify(out).includes("SENTINEL"));
  assert.deepEqual([out[0].live?.state, out[0].live?.detail, out[0].live?.since_ms], ["waiting", "permission", 10]);
  // The question form waits behind the permission, then the input form.
  const replied = live.v2({ type: "permission.replied", data: { sessionID: "ses_8charsok", requestID: "per_1", reply: "once" } }, 20);
  assert.deepEqual([replied[0].live?.state, replied[0].live?.detail], ["waiting", "question"]);
  const answered = live.v2({ type: "form.replied", data: { id: "frm_1", sessionID: "ses_8charsok" } }, 30);
  assert.deepEqual([answered[0].live?.state, answered[0].live?.detail], ["waiting", "input"]);
  const cancelled = live.v2({ type: "form.cancelled", data: { id: "frm_3", sessionID: "ses_8charsok" } }, 40);
  assert.deepEqual([cancelled[0].live?.state, cancelled[0].live?.detail, cancelled[0].live?.since_ms], ["working", "generating", 10]);
  // None of the ended asks comes back from a later list.
  assert.deepEqual(
    live.seed("ses_8charsok", { permissions: [ask("per_1", "ses_8charsok")], forms: [form("frm_1", "ses_8charsok", "question"), form("frm_3", "ses_8charsok")] }, 50),
    [],
  );
});

test("a seed's status fills in only a session with no evidence, and never removes a wait", () => {
  const live = createLiveState({ instance: "test-instance", directory: "/work" });
  const fresh = live.seed("ses_idle0001", { status: "idle" }, 1);
  assert.deepEqual([fresh[0].live?.state, fresh[0].live?.detail], ["ready", undefined]);
  assert.deepEqual(live.seed("ses_idle0001", { status: "running" }, 2), [], "the session has evidence now");
  const running = live.seed("ses_busy0001", { status: "running" }, 3);
  assert.deepEqual([running[0].live?.state, running[0].live?.detail, running[0].live?.since_ms], ["working", "generating", 3]);
  assert.deepEqual(live.seed("ses_none0001", { status: "SENTINEL" }, 4), [], "a word it does not know");
  assert.deepEqual(live.seed("ses_none0001", { permissions: "not a list", forms: {} }, 4), []);
  assert.deepEqual(live.seed("not an id", { status: "running" }, 4), []);
  live.v2({ type: "permission.asked", data: { id: "per_1", sessionID: "ses_busy0001" } }, 5);
  assert.deepEqual(live.seed("ses_busy0001", { permissions: [] }, 6), [], "an empty list removes nothing");
  // A rejection ends every pending permission of the session, even one
  // the list names and the instance never saw.
  live.v2({ type: "permission.replied", data: { sessionID: "ses_busy0001", requestID: "per_2", reply: "reject" } }, 7);
  assert.deepEqual(live.seed("ses_busy0001", { permissions: [ask("per_2", "ses_busy0001")] }, 8), []);
  const ended = live.v2({ type: "session.execution.interrupted", data: { sessionID: "ses_busy0001", reason: "shutdown" } }, 9);
  assert.equal(ended[0].live?.state, "exited");
  assert.deepEqual(live.seed("ses_busy0001", { permissions: [ask("per_3", "ses_busy0001")] }, 10), [], "an ended session waits on nothing");
});

/** A stand-in for the server plugin context's two lists, which checks
 * that each is called as a method and records what it was asked. */
function standInLists(sessions: string[], asks: Record<string, unknown[]>, options: { failFor?: string; pageSize?: number } = {}) {
  const calls: string[] = [];
  const pageSize = options.pageSize ?? 2;
  const session = {
    async list(this: unknown, input: { directory?: string; limit?: number; cursor?: string }) {
      assert.equal(this, session, "session.list is called as a method");
      calls.push(`session.list ${JSON.stringify(input)}`);
      const start = input.cursor === undefined ? 0 : Number(input.cursor);
      const data = sessions.slice(start, start + Math.min(pageSize, input.limit ?? pageSize)).map((id) => ({ id, title: "SENTINEL title", directory: "/work" }));
      const next = start + data.length < sessions.length ? String(start + data.length) : undefined;
      return { data, cursor: next === undefined ? {} : { next } };
    },
  };
  const permission = {
    async list(this: unknown, input: { sessionID: string }) {
      assert.equal(this, permission, "permission.list is called as a method");
      calls.push(`permission.list ${input.sessionID}`);
      if (input.sessionID === options.failFor) throw new Error("SENTINEL failure");
      return asks[input.sessionID] ?? [];
    },
  };
  return { api: { session, permission, location: { directory: "/work" } }, calls };
}

test("pending asks are read from the location's recent sessions through a stand-in context", async () => {
  const reads = new Set<string>();
  const pending = watched([ask("per_1", "ses_00000002")], reads);
  const { api, calls } = standInLists(["ses_00000001", "ses_00000002", "ses_00000003", "ses_00000004", "ses_00000005"], { ses_00000002: pending }, { failFor: "ses_00000003" });
  const seen: Array<[string, Listed]> = [];
  await readPending(api, "/work", (sessionID, listed) => seen.push([sessionID, listed]));
  assert.deepEqual(calls, [
    'session.list {"directory":"/work","limit":50}',
    'session.list {"cursor":"2"}',
    'session.list {"cursor":"4"}',
    "permission.list ses_00000001",
    "permission.list ses_00000002",
    "permission.list ses_00000003",
    "permission.list ses_00000004",
    "permission.list ses_00000005",
  ]);
  assert.deepEqual(seen.map(([sessionID]) => sessionID), ["ses_00000002"], "a failed call is skipped; an empty list says nothing");
  assert.deepEqual([...reads], [], "the asks are passed on unread");

  // Fed to the reducer, the session reads waiting.
  const live = createLiveState({ instance: "test-instance", directory: "/work" });
  const out = seen.flatMap(([sessionID, listed]) => live.seed(sessionID, listed, 100));
  assert.deepEqual(out.map((payload) => [payload.session_id, payload.live?.state, payload.live?.detail]), [["ses_00000002", "waiting", "permission"]]);
  assert.ok(!JSON.stringify(out).includes("SENTINEL"));
});

test("reading pending asks stops at its limit, when the plugin unloads, and without the calls", async () => {
  const many = Array.from({ length: MAX_LISTED_SESSIONS + 20 }, (_, n) => `ses_${String(n).padStart(8, "0")}`);
  const limited = standInLists(many, {}, { pageSize: 50 });
  await readPending(limited.api, "/work", () => {});
  assert.equal(limited.calls.filter((call) => call.startsWith("permission.list")).length, MAX_LISTED_SESSIONS);

  const unloading = standInLists(["ses_00000001", "ses_00000002"], { ses_00000001: [ask("per_1", "ses_00000001")] });
  let active = true;
  const seen: string[] = [];
  await readPending(unloading.api, "/work", (sessionID) => seen.push(sessionID), () => {
    const now = active;
    active = false;
    return now;
  });
  assert.deepEqual(unloading.calls, ['session.list {"directory":"/work","limit":50}']);
  assert.deepEqual(seen, []);

  for (const api of [undefined, {}, { session: {} }, { session: { list: async () => [] } }, { permission: { list: async () => [] } }]) {
    await readPending(api, "/work", () => assert.fail("nothing to read"));
  }
  const noDirectory = standInLists(["ses_00000001"], { ses_00000001: [ask("per_1", "ses_00000001")] });
  await readPending(noDirectory.api, undefined, () => assert.fail("no location"));
  assert.deepEqual(noDirectory.calls, []);
  const failing = { session: { list: async () => { throw new Error("SENTINEL"); } }, permission: { list: async () => [] } };
  await readPending(failing, "/work", () => assert.fail("the list failed"));
  const odd = { session: { list: async () => ({ data: [{ id: "has space" }, { id: 7 }, {}] }) }, permission: { list: async () => assert.fail("no session to ask") } };
  await readPending(odd, "/work", () => assert.fail("no session"));
});
