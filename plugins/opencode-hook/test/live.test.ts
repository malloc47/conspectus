// The live-state reducer (`src/live.ts`) against the shared event
// scripts in `tests/fixtures/hooks/opencode-live/`. Each script is an
// opencode major, a header with the plugin instance's options, and steps:
// an input (an `event`, the 1.x `compacting` hook, a `heartbeat`, or
// `dispose`), the payloads the reducer must `emit` after it, in order,
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
  MAX_SESSIONS,
  V1_EVENTS,
  V2_EVENTS,
  createLiveState,
  newInstanceId,
  parseProcStartTime,
  type HookPayload,
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
  const order = ["at_ms", "note", "event", "compacting", "heartbeat", "dispose", "emit", "shown"];
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
  for (const script of scripts()) for (const step of script.steps) walk(step.event, []);
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
