// The Pi extension (`../conspectus-hook.ts`) under node, with no Pi.
//
// The state machine replays the shared event scripts in
// `tests/fixtures/hooks/pi-live/`. Each script is a header with the
// extension load's options and steps: an input (a Pi event `on` with its
// `event`, and for `session_start` the `ctx` the extension reads; a due
// `timer`; or none for a step that only checks), the payloads the machine
// must `emit` after it, in order, when it asks to be called back
// (`flush_at`, `heartbeat_at`), and what each session it names is `shown`
// as. The Rust tests feed the same payloads through `conspectus hook
// write pi`'s parser and the live-state store, and check `shown` there
// too.
//
// The default export runs against a stand-in for Pi's extension API and a
// stand-in writer (`CONSPECTUS_HOOK_BIN`), so the handlers, the spawn, and
// the timers run as they would in Pi. Nothing here was checked against
// Pi's own types.
//
// Run with Node 22.6 or later: `just pi-plugin-test`. With
// `PI_LIVE_UPDATE=1` the run rewrites each step's `emit`, `flush_at`, and
// `heartbeat_at` from the machine instead of comparing them; review the
// diff before keeping it.

import { test } from "node:test";
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import extension, {
  testOnly,
  type HookPayload,
  type LiveSnapshot,
  type Machine,
  type Output,
  type SessionContext,
} from "../conspectus-hook.ts";

const { COALESCE_MS, EVENTS, EXTENSION_VERSION, HEARTBEAT_S, SUBAGENT_TOOLS, classifyError, createMachine, newInstanceId, parseProcStartTime } =
  testOnly;

const EXTENSION = fileURLToPath(new URL("../conspectus-hook.ts", import.meta.url));
const FIXTURES = fileURLToPath(new URL("../../../tests/fixtures/hooks/pi-live/", import.meta.url));
const UPDATE = process.env.PI_LIVE_UPDATE === "1";

type Header = { synthetic: boolean; scenario: string; instance: string; pid?: number; pid_start?: number };

type Shown = {
  state: string;
  detail?: string;
  since_ms?: number;
  subagents?: number;
  confidence?: string;
  liveness?: string;
};

type Step = {
  at_ms: number;
  note?: string;
  load?: { instance: string };
  on?: string;
  event?: unknown;
  ctx?: SessionContext;
  timer?: "flush" | "heartbeat";
  emit?: HookPayload[];
  flush_at?: number;
  heartbeat_at?: number;
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

function machine(header: Header, instance: string = header.instance): Machine {
  return createMachine({ instance, pid: header.pid, pidStart: header.pid_start });
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
    if (expect.since_ms !== undefined) assert.equal(live.since_ms, expect.since_ms, `${place}: ${session} since_ms`);
  }
}

/** The script with each step's keys in reading order. */
function rewrite(script: Script): void {
  const order = ["at_ms", "note", "load", "on", "event", "ctx", "timer", "emit", "flush_at", "heartbeat_at", "shown"];
  const rank = (key: string) => (order.includes(key) ? order.indexOf(key) : order.length);
  const ordered = script.steps.map((step) => Object.fromEntries(Object.entries(step).sort(([a], [b]) => rank(a) - rank(b))));
  const lines = [script.header, ...ordered].map((line) => JSON.stringify(line));
  writeFileSync(`${FIXTURES}${script.name}.jsonl`, lines.join("\n") + "\n");
}

for (const script of scripts()) {
  test(`${script.name}: ${script.header.scenario}`, () => {
    assert.equal(script.header.synthetic, true, "every script is labeled synthetic");
    let live = machine(script.header);
    let instance = script.header.instance;
    let seq = 0;
    let due: { flush?: number; heartbeat?: number } = {};
    const latest = new Map<string, LiveSnapshot>();
    script.steps.forEach((step, index) => {
      const place = `${script.name} step ${index + 1}${step.note ? ` (${step.note})` : ""}`;
      if (step.load) {
        live = machine(script.header, step.load.instance);
        instance = step.load.instance;
        seq = 0;
        due = {};
      }
      let output: Output | undefined;
      if (step.on !== undefined) {
        output = live.on(step.on, step.event ?? {}, step.at_ms, step.ctx);
      } else if (step.timer !== undefined) {
        assert.equal(step.at_ms, due[step.timer], `${place}: the ${step.timer} timer fires when it was due`);
        output = step.timer === "flush" ? live.flush(step.at_ms) : live.heartbeat(step.at_ms);
      }
      if (output === undefined) {
        assert.equal(step.emit, undefined, `${place}: a check-only step emits nothing`);
      } else {
        if (UPDATE) {
          step.emit = output.emit;
          if (output.flush_at === undefined) delete step.flush_at;
          else step.flush_at = output.flush_at;
          if (output.heartbeat_at === undefined) delete step.heartbeat_at;
          else step.heartbeat_at = output.heartbeat_at;
        }
        assert.deepEqual(output.emit, step.emit, place);
        assert.equal(output.flush_at, step.flush_at, `${place}: flush_at`);
        assert.equal(output.heartbeat_at, step.heartbeat_at, `${place}: heartbeat_at`);
        due = { flush: output.flush_at, heartbeat: output.heartbeat_at };
        for (const payload of output.emit) {
          assert.ok(!JSON.stringify(payload).includes("SENTINEL"), `${place}: payload text left the extension`);
          assert.equal(payload.live.instance, instance, `${place}: instance`);
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

test("every handled event has a script, and nothing streams or steers Pi", () => {
  const seen = new Set<string>();
  for (const script of scripts()) for (const step of script.steps) if (step.on) seen.add(step.on);
  for (const name of EVENTS) assert.ok(seen.has(name), name);
  for (const name of [
    "message_start",
    "message_update",
    "tool_execution_update",
    "provider_stream_event",
    "tool_call",
    "tool_result",
    "context",
    "before_agent_start",
    "input",
    "turn_start",
    "turn_end",
  ]) {
    assert.ok(!(EVENTS as readonly string[]).includes(name), name);
  }
});

test("the scripts plant a sentinel in every payload-bearing field", () => {
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
    "messages",
    "content",
    "text",
    "thinking",
    "errorMessage",
    "args",
    "command",
    "result",
    "details",
    "title",
    "toolName",
    "preparation",
    "branchEntries",
    "compactionEntry",
    "summary",
    "headers",
    "targetSessionFile",
  ]) {
    assert.ok(fields.has(field), field);
  }
});

const SESSION = "0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d";
const FILE = "/home/user/.pi/agent/sessions/--work--/2026-11-11T10-00-00-000Z_0199f0a2-7c1e-7d3a-9b2e-5f1c2a3b4c5d.jsonl";
const tui = (extra: SessionContext = {}): SessionContext => ({
  mode: "tui",
  idle: true,
  session_id: SESSION,
  session_file: FILE,
  cwd: "/work",
  ...extra,
});

test("events it does not handle, and inputs that are not events, change nothing", () => {
  const live = createMachine({ instance: "test-instance" });
  assert.deepEqual(live.on("agent_start", {}, 1), { emit: [] }, "nothing before a session starts");
  live.on("session_start", { reason: "startup" }, 1, tui());
  for (const name of ["message_update", "tool_execution_update", "tool_call", "turn_start", "unknown"]) {
    assert.deepEqual(live.on(name, { delta: "SENTINEL" }, 2).emit, [], name);
  }
  for (const event of [null, undefined, 7, "agent_settled", [], { message: "SENTINEL" }]) {
    assert.deepEqual(live.on("message_end", event, 3).emit, []);
    assert.deepEqual(live.on("tool_execution_start", event, 3).emit, []);
    assert.deepEqual(live.on("ui_prompt_start", event, 3).emit, []);
    assert.deepEqual(live.on("after_provider_response", event, 3).emit, []);
  }
  assert.deepEqual(live.flush(4), { emit: [] });
  assert.deepEqual(live.heartbeat(5), { emit: [] }, "a ready session gets no heartbeat");
});

test("ids and paths that do not look like ones are refused", () => {
  for (const sessionID of ["SENTINEL text with spaces", "", "x".repeat(257), 42, "bell\u0007id", undefined]) {
    const live = createMachine({ instance: "test-instance" });
    assert.deepEqual(live.on("session_start", { reason: "startup" }, 1, tui({ session_id: sessionID })).emit, [], String(sessionID));
  }
  const live = createMachine({ instance: "test-instance" });
  const [named] = live.on("session_start", { reason: "SENTINEL reason" }, 1, tui({ cwd: "/work\nSENTINEL" })).emit;
  assert.equal(named.hook_event_name, "session_start:other", "a reason Pi does not document is not copied");
  assert.equal(named.cwd, undefined, "a cwd with a control character is dropped");
  live.on("agent_start", {}, 2);
  for (const call of ["call with space", "", 7]) {
    assert.deepEqual(live.on("tool_execution_start", { toolCallId: call, toolName: "bash" }, 3), { emit: [], heartbeat_at: 60_002 });
  }
});

test("a run that starts before the session is idle shows working", () => {
  const live = createMachine({ instance: "test-instance", pid: 4242 });
  const [payload] = live.on("session_start", { reason: "reload" }, 10, tui({ idle: false })).emit;
  assert.equal(payload.live.state, "working");
  assert.equal(payload.live.detail, "generating");
  assert.equal(payload.live.pid, 4242);
  assert.equal(payload.live.pid_start, undefined);
  assert.equal(payload.transcript_path, FILE);
});

test("error messages are classified and never copied", () => {
  const cases: Array<[unknown, string]> = [
    ["429 Too Many Requests", "rate_limit"],
    ['{"type":"rate_limit_error","message":"Number of request tokens has exceeded your per-minute rate limit"}', "rate_limit"],
    ["You exceeded your current quota", "rate_limit"],
    ["RESOURCE_EXHAUSTED", "rate_limit"],
    ['529 {"type":"overloaded_error","message":"Overloaded"}', "overloaded"],
    ["503 Service Unavailable", "overloaded"],
    ["401 Unauthorized", "auth"],
    ["invalid x-api-key", "auth"],
    ["OAuth token expired; run /login", "auth"],
    ["prompt is too long: 212000 tokens > 200000 maximum", "context_overflow"],
    ["This model's maximum context length is 128000 tokens", "context_overflow"],
    ["fetch failed", "network"],
    ["read ECONNRESET", "network"],
    ["Request timed out", "network"],
    ["500 Internal Server Error", "provider"],
    ["502 Bad Gateway", "provider"],
    ["the model refused", "unknown"],
    ["", "unknown"],
    [undefined, "unknown"],
    [{ message: "429" }, "unknown"],
  ];
  for (const [message, expected] of cases) assert.equal(classifyError(message), expected, String(message));
  assert.equal(classifyError("x".repeat(10_000) + " 429"), "unknown", "only the start of a long message is read");
});

test("the start time is field twenty-two counted past the command name", () => {
  const stat = (command: string, start: number) => `4242 (${command}) S 1 4242 4242 0 -1 4194560 100 0 0 0 1 2 0 0 20 0 1 0 ${start} 1000 10`;
  assert.equal(parseProcStartTime(stat("pi", 987654)), 987654);
  assert.equal(parseProcStartTime(stat("a (weird) name", 31337)), 31337);
  assert.equal(parseProcStartTime("4242 (short) S 1"), undefined);
  assert.equal(parseProcStartTime("no parentheses"), undefined);
});

test("instance ids look like ids and differ", () => {
  const ids = new Set(Array.from({ length: 100 }, () => newInstanceId()));
  assert.equal(ids.size, 100);
  for (const value of ids) assert.match(value, /^[0-9a-z]+-[0-9a-f]{16}$/);
  assert.equal(newInstanceId(0, () => 0), "0-0000000000000000");
});

test("the constants the documentation states", () => {
  assert.equal(EXTENSION_VERSION, "2");
  assert.equal(HEARTBEAT_S, 60);
  assert.equal(COALESCE_MS, 500);
  assert.deepEqual([...SUBAGENT_TOOLS], ["subagent"]);
  const source = readFileSync(EXTENSION, "utf8");
  assert.ok(source.startsWith("// conspectus-hook:"), "the ownership marker is the first line");
  assert.ok(source.includes(`const EXTENSION_VERSION = "${EXTENSION_VERSION}";`), "hook status reads the version from this line");
});

// The default export, as Pi runs it.

type Handler = (event: unknown, ctx: unknown) => unknown;

/** A stand-in for Pi's extension API that records what the extension
 * registers, and fails on anything else it touches. */
function standInPi(): { api: unknown; handlers: Map<string, Handler> } {
  const handlers = new Map<string, Handler>();
  const api = new Proxy(
    {
      on(name: string, handler: Handler) {
        assert.ok(!handlers.has(name), `${name} registered once`);
        handlers.set(name, handler);
      },
    },
    {
      get(target, key) {
        assert.equal(key, "on", `the extension touched pi.${String(key)}`);
        return Reflect.get(target, key);
      },
    },
  );
  return { api, handlers };
}

function piContext(extra: Record<string, unknown> = {}, idle = true): unknown {
  return {
    mode: "tui",
    cwd: "/fallback",
    isIdle: () => idle,
    sessionManager: {
      getSessionId: () => SESSION,
      getSessionFile: () => FILE,
      getCwd: () => "/work",
    },
    ...extra,
  };
}

/** A scratch directory with a stand-in `conspectus` that keeps each
 * payload it reads, with its arguments and the version variable. */
function standInWriter(): { dir: string; bin: string; payloads: () => Array<{ args: string; version: string; payload: HookPayload }> } {
  const dir = mkdtempSync(join(tmpdir(), "pi-hook-test-"));
  const bin = join(dir, "conspectus");
  writeFileSync(
    bin,
    `#!/bin/sh\nout="${dir}/out-$$"\nprintf '%s\\n%s\\n' "$*" "$CONSPECTUS_PI_HOOK_VERSION" > "$out.meta"\ncat > "$out.tmp" && mv "$out.tmp" "$out.json"\n`,
  );
  chmodSync(bin, 0o755);
  const payloads = () =>
    readdirSync(dir)
      .filter((file) => file.endsWith(".json"))
      .map((file) => {
        const [args, version] = readFileSync(join(dir, file.replace(/\.json$/, ".meta")), "utf8").split("\n");
        return { args, version, payload: JSON.parse(readFileSync(join(dir, file), "utf8")) as HookPayload };
      })
      .sort((a, b) => a.payload.live.seq - b.payload.live.seq);
  return { dir, bin, payloads };
}

async function eventually<T>(read: () => T, done: (value: T) => boolean, what: string): Promise<T> {
  const deadline = Date.now() + 5000;
  for (;;) {
    const value = read();
    if (done(value)) return value;
    if (Date.now() > deadline) assert.fail(`timed out waiting for ${what}: ${JSON.stringify(value)}`);
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
}

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

test("the default export registers the handled events and writes what the machine emits", async () => {
  const writer = standInWriter();
  const previous = process.env.CONSPECTUS_HOOK_BIN;
  process.env.CONSPECTUS_HOOK_BIN = writer.bin;
  try {
    const pi = standInPi();
    extension(pi.api as never);
    assert.deepEqual([...pi.handlers.keys()].sort(), [...EVENTS].sort());
    const on = (name: string, event: unknown, ctx: unknown = {}) => {
      // Handlers return at once: nothing is awaited.
      assert.equal(pi.handlers.get(name)?.(event, ctx), undefined, name);
    };

    on("session_start", { reason: "startup" }, piContext());
    let written = await eventually(writer.payloads, (all) => all.length === 1, "the session start");
    assert.equal(written[0].args, "hook write pi");
    assert.equal(written[0].version, EXTENSION_VERSION);
    assert.equal(written[0].payload.hook_event_name, "session_start:startup");
    assert.equal(written[0].payload.session_id, SESSION);
    assert.equal(written[0].payload.cwd, "/work");
    assert.equal(written[0].payload.transcript_path, FILE);
    assert.equal(written[0].payload.live.pid, process.pid);
    assert.equal(written[0].payload.live.state, "ready");
    if (process.platform === "linux") assert.ok(Number.isInteger(written[0].payload.live.pid_start));

    on("agent_start", {});
    written = await eventually(writer.payloads, (all) => all.length === 2, "the run start");
    assert.equal(written[1].payload.live.state, "working");

    // A message's end never starts a writer.
    on("message_end", { message: { role: "assistant", stopReason: "toolUse", content: [{ type: "text", text: "SENTINEL" }] } });
    on("tool_execution_start", { toolCallId: "call_1", toolName: "bash", args: { command: "SENTINEL" } });
    await sleep(COALESCE_MS / 2);
    assert.equal(writer.payloads().length, 2, "a working sub-state is held");
    written = await eventually(writer.payloads, (all) => all.length === 3, "the held change");
    assert.equal(written[2].payload.live.detail, "tool");
    assert.equal(written[2].payload.hook_event_name, "live:tool_execution_start");

    on("ui_prompt_start", { kind: "confirm", title: "SENTINEL" });
    written = await eventually(writer.payloads, (all) => all.length === 4, "the wait");
    assert.equal(written[3].payload.live.state, "waiting");
    on("ui_prompt_end", { kind: "confirm", title: "SENTINEL" });
    on("session_shutdown", { reason: "quit" });
    written = await eventually(writer.payloads, (all) => all.length === 6, "the exit");
    assert.deepEqual(
      written.map(({ payload }) => [payload.live.seq, payload.live.state, payload.live.detail]),
      [
        [1, "ready", undefined],
        [2, "working", "generating"],
        [3, "working", "tool"],
        [4, "waiting", "permission"],
        [5, "working", "tool"],
        [6, "exited", "ended"],
      ],
    );
    for (const { payload } of written) assert.ok(!JSON.stringify(payload).includes("SENTINEL"));
    await sleep(COALESCE_MS + 100);
    assert.equal(writer.payloads().length, 6, "nothing after the exit");
  } finally {
    if (previous === undefined) delete process.env.CONSPECTUS_HOOK_BIN;
    else process.env.CONSPECTUS_HOOK_BIN = previous;
    rmSync(writer.dir, { recursive: true, force: true });
  }
});

test("print, JSON, and RPC modes, --no-session runs, and broken contexts write nothing", async () => {
  const writer = standInWriter();
  const previous = process.env.CONSPECTUS_HOOK_BIN;
  process.env.CONSPECTUS_HOOK_BIN = writer.bin;
  try {
    const pi = standInPi();
    extension(pi.api as never);
    const start = pi.handlers.get("session_start")!;
    for (const mode of ["print", "json", "rpc"]) start({ reason: "startup" }, piContext({ mode }));
    start({ reason: "startup" }, piContext({ sessionManager: { getSessionId: () => SESSION, getSessionFile: () => undefined } }));
    start({ reason: "startup" }, piContext({ sessionManager: { getSessionId: () => { throw new Error("stale ctx"); } } }));
    start({ reason: "startup" }, undefined);
    start(undefined, null);
    pi.handlers.get("agent_start")!({}, undefined);
    pi.handlers.get("ui_prompt_start")!({ kind: "confirm" }, undefined);
    await sleep(COALESCE_MS + 200);
    assert.deepEqual(writer.payloads(), []);
  } finally {
    if (previous === undefined) delete process.env.CONSPECTUS_HOOK_BIN;
    else process.env.CONSPECTUS_HOOK_BIN = previous;
    rmSync(writer.dir, { recursive: true, force: true });
  }
});

/** Run `body` in a fresh node process that has loaded the extension into
 * a stand-in Pi as `on(name, event, ctx)`, with `env`. */
function inChild(body: string, env: Record<string, string>): { status: number | null; stdout: string; stderr: string; ms: number } {
  const script = `
    const { default: extension } = await import(${JSON.stringify(EXTENSION)});
    const handlers = new Map();
    extension({ on: (name, handler) => handlers.set(name, handler) });
    const on = (name, event, ctx) => handlers.get(name)(event, ctx);
    const ctx = {
      mode: "tui",
      isIdle: () => true,
      sessionManager: { getSessionId: () => ${JSON.stringify(SESSION)}, getSessionFile: () => ${JSON.stringify(FILE)}, getCwd: () => "/work" },
    };
    ${body}
  `;
  const begun = Date.now();
  const result = spawnSync(process.execPath, ["--experimental-strip-types", "--no-warnings", "--input-type=module", "-e", script], {
    env: { PATH: process.env.PATH ?? "/usr/bin:/bin", ...env },
    encoding: "utf8",
    timeout: 20_000,
  });
  return { status: result.status, stdout: result.stdout, stderr: result.stderr, ms: Date.now() - begun };
}

test("its timers never keep Pi running, and a quit's payload outlives Pi", async () => {
  const writer = standInWriter();
  try {
    // A working session holds a change and has a heartbeat due in a
    // minute; the process still ends as soon as its own work is done.
    const running = inChild(
      `on("session_start", { reason: "startup" }, ctx);
       on("agent_start", {}, ctx);
       on("tool_execution_start", { toolCallId: "call_1", toolName: "bash" }, ctx);`,
      { CONSPECTUS_HOOK_BIN: writer.bin },
    );
    assert.equal(running.status, 0, running.stderr);
    assert.ok(running.ms < HEARTBEAT_S * 1000 / 4, `exited after ${running.ms} ms`);
    await eventually(writer.payloads, (all) => all.length === 2, "the two sent before the process ended");

    // Pi may exit right after its shutdown handlers run.
    const quit = inChild(
      `on("session_start", { reason: "startup" }, ctx);
       on("session_shutdown", { reason: "quit" }, ctx);
       process.exit(0);`,
      { CONSPECTUS_HOOK_BIN: writer.bin },
    );
    assert.equal(quit.status, 0, quit.stderr);
    const written = await eventually(writer.payloads, (all) => all.some(({ payload }) => payload.live.state === "exited"), "the exit");
    assert.ok(written.some(({ payload }) => payload.live.state === "exited" && payload.hook_event_name === "live:session_shutdown"));
  } finally {
    rmSync(writer.dir, { recursive: true, force: true });
  }
});

test("a missing writer is ignored without a word", () => {
  const missing = inChild(
    `on("session_start", { reason: "startup" }, ctx);
     on("agent_start", {}, ctx);
     on("session_shutdown", { reason: "quit" }, ctx);
     await new Promise((resolve) => setTimeout(resolve, 200));`,
    { CONSPECTUS_HOOK_BIN: "/nonexistent/conspectus" },
  );
  assert.equal(missing.status, 0, missing.stderr);
  assert.equal(missing.stdout, "");
  assert.equal(missing.stderr, "");
});
