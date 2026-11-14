// The opencode 2.x entrypoints (`src/v2/`) under node, with stand-ins for
// opencode's plugin context and for `conspectus`, so the glue between
// opencode and the reducer runs as it would in opencode. Nothing here was
// checked against a running opencode; the stand-ins follow the plugin
// package's published types (`@opencode/plugin` 2.0.32) and its server
// plugin documentation.
//
// The entrypoints import the `.js` files `tsc` writes, so a resolve hook
// loads each from its `.ts` source. It needs `module.registerHooks`
// (Node 22.15 or later); on an older Node these tests are skipped.

import { test } from "node:test";
import assert from "node:assert/strict";
import * as nodeModule from "node:module";
import { chmodSync, existsSync, mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { V2_EVENTS, type HookPayload } from "../src/live.ts";

type Resolve = (specifier: string, context: { parentURL?: string }, next: (specifier: string, context: unknown) => unknown) => unknown;
const registerHooks = (nodeModule as unknown as { registerHooks?: (hooks: { resolve: Resolve }) => void }).registerHooks;
registerHooks?.({
  resolve(specifier, context, next) {
    if (specifier.startsWith(".") && specifier.endsWith(".js") && context.parentURL?.endsWith(".ts")) {
      const source = new URL(specifier.replace(/\.js$/, ".ts"), context.parentURL);
      if (existsSync(fileURLToPath(source))) return next(source.href, context);
    }
    return next(specifier, context);
  },
});
const skip = registerHooks ? false : "needs module.registerHooks (Node 22.15 or later)";

/** A scratch directory with a stand-in `conspectus` that keeps each
 * payload it reads. The entrypoints read `CONSPECTUS_HOOK_BIN` when they
 * load, so it is set before any of them is imported. */
function standInWriter(): { dir: string; bin: string; payloads: () => HookPayload[] } {
  const dir = mkdtempSync(join(tmpdir(), "opencode-hook-test-"));
  const bin = join(dir, "conspectus");
  writeFileSync(bin, `#!/bin/sh\nout="${dir}/out-$(date +%s%N)-$$"\ncat > "$out.tmp" && mv "$out.tmp" "$out.json"\n`);
  chmodSync(bin, 0o755);
  const payloads = () =>
    readdirSync(dir)
      .filter((file) => file.endsWith(".json"))
      .sort()
      .map((file) => JSON.parse(readFileSync(join(dir, file), "utf8")) as HookPayload);
  return { dir, bin, payloads };
}

const writer = standInWriter();
process.env.CONSPECTUS_HOOK_BIN = writer.bin;
process.on("exit", () => rmSync(writer.dir, { recursive: true, force: true }));

async function eventually<T>(read: () => T, done: (value: T) => boolean, what: string): Promise<T> {
  const deadline = Date.now() + 5000;
  for (;;) {
    const value = read();
    if (done(value)) return value;
    if (Date.now() > deadline) assert.fail(`timed out waiting for ${what}: ${JSON.stringify(value)}`);
    await new Promise((resolve) => setTimeout(resolve, 20));
  }
}

/** A stand-in for `ctx.event.subscribe`'s stream: events pushed into it
 * come out in order until the subscriber's signal aborts. */
function eventStream() {
  const queue: unknown[] = [];
  let wake: (() => void) | undefined;
  return {
    push(event: unknown) {
      queue.push(event);
      wake?.();
    },
    async *events(signal?: AbortSignal) {
      while (!signal?.aborted) {
        const next = queue.shift();
        if (next !== undefined) {
          yield next;
          continue;
        }
        await new Promise<void>((resolve) => {
          wake = resolve;
          signal?.addEventListener("abort", () => resolve(), { once: true });
        });
        wake = undefined;
      }
    },
  };
}

const ask = (id: string, sessionID: string) => ({
  id,
  sessionID,
  action: "bash",
  resources: ["SENTINEL resource"],
  metadata: { command: "SENTINEL metadata" },
  message: "SENTINEL message",
});

const forSession = (sessionID: string) => (payload: HookPayload) => payload.session_id === sessionID && payload.live !== undefined;

test("the server entrypoint reports a permission ask pending when it starts", { skip }, async () => {
  const { default: plugin } = await import("../src/v2/server.ts");
  const stream = eventStream();
  const subscribed: unknown[] = [];
  const asked: string[] = [];
  const ctx = {
    location: { directory: "/work" },
    event: {
      subscribe(input: { types?: unknown; signal?: AbortSignal }) {
        subscribed.push(input.types);
        return stream.events(input.signal);
      },
    },
    session: {
      async list(input: unknown) {
        asked.push(`session.list ${JSON.stringify(input)}`);
        return { data: [{ id: "ses_pending001" }, { id: "ses_quiet00001" }], cursor: {} };
      },
    },
    permission: {
      async list(input: { sessionID: string }) {
        asked.push(`permission.list ${input.sessionID}`);
        return input.sessionID === "ses_pending001" ? [ask("per_1", "ses_pending001")] : [];
      },
    },
  };
  const cleanup = (await plugin.setup(ctx as never)) as () => void;
  assert.deepEqual(subscribed, [[...V2_EVENTS]]);

  let written = await eventually(writer.payloads, (all) => all.some(forSession("ses_pending001")), "the pending ask");
  const seeded = written.filter(forSession("ses_pending001"));
  assert.deepEqual(
    seeded.map((payload) => [payload.hook_event_name, payload.live?.state, payload.live?.detail, payload.cwd]),
    [["seed", "waiting", "permission", "/work"]],
  );
  assert.equal(seeded[0].live?.pid, process.pid);
  assert.equal(seeded[0].attribution, undefined, "the server entrypoint attributes nothing");
  assert.deepEqual(asked, ['session.list {"directory":"/work","limit":50}', "permission.list ses_pending001", "permission.list ses_quiet00001"]);
  assert.ok(!written.some(forSession("ses_quiet00001")), "a session with nothing pending says nothing");

  // The answer arrives as an event.
  stream.push({ type: "permission.replied", data: { sessionID: "ses_pending001", requestID: "per_1", reply: "once" }, location: { directory: "/work" } });
  written = await eventually(writer.payloads, (all) => all.filter(forSession("ses_pending001")).length === 2, "the answer");
  assert.deepEqual(
    written.filter(forSession("ses_pending001")).map((payload) => [payload.live?.state, payload.live?.detail]),
    [["waiting", "permission"], ["working", "generating"]],
  );

  // Unloading ends the stream and the session's turn.
  cleanup();
  written = await eventually(writer.payloads, (all) => all.filter(forSession("ses_pending001")).length === 3, "the unload");
  assert.deepEqual(
    written.filter(forSession("ses_pending001")).map((payload) => payload.live?.state),
    ["waiting", "working", "exited"],
  );
  assert.ok(!JSON.stringify(written).includes("SENTINEL"));
});

/** A stand-in for the TUI plugin context: an event stream to push into,
 * a route to set, session stores, and the service's plugin list. */
function standInClient(answer: () => unknown) {
  const listeners = new Set<(event: { details: unknown }) => void>();
  const asked: unknown[] = [];
  const state = { route: { type: "home" } as { type: string; sessionID?: string } };
  const permissions: Record<string, unknown[]> = {};
  const session = {
    status(this: unknown, sessionID: string) {
      assert.equal(this, session, "status is called as a method");
      return sessionID in permissions ? "running" : "idle";
    },
    family: (sessionID: string) => [sessionID],
    permission: { list: (sessionID: string) => permissions[sessionID] },
    form: { list: () => [] },
  };
  const plugin = {
    async list(this: unknown, input: unknown) {
      assert.equal(this, plugin, "plugin.list is called as a method");
      asked.push(input);
      return answer();
    },
  };
  const ctx = {
    location: { directory: "/work" },
    data: {
      listen(handler: (event: { details: unknown }) => void) {
        listeners.add(handler);
        return () => listeners.delete(handler);
      },
      session,
    },
    ui: { router: { current: () => state.route } },
    client: { plugin },
  };
  return {
    ctx,
    asked,
    permissions,
    show(sessionID: string) {
      state.route = { type: "session", sessionID };
    },
    push(details: unknown) {
      for (const listener of listeners) listener({ details });
    },
  };
}

const named = (sessionID: string) => (payload: HookPayload) => payload.session_id === sessionID;

test("the TUI entrypoint reports the sessions its client shows when no server entrypoint does", { skip }, async () => {
  const { default: plugin } = await import("../src/v2/tui.ts");
  const client = standInClient(() => ({ location: { directory: "/work" }, data: [{ id: "conspectus-hook", features: { tui: true }, state: { status: "active" } }] }));
  client.permissions.ses_fallback01 = [ask("per_t1", "ses_fallback01")];
  const cleanup = (await plugin.setup(client.ctx as never)) as () => void;
  try {
    await eventually(() => client.asked, (asked) => asked.length === 1, "the plugin list");
    assert.deepEqual(client.asked, [{ location: { directory: "/work" } }]);
    client.show("ses_fallback01");
    client.push({ type: "session.step.started", data: { sessionID: "ses_fallback01" }, location: { directory: "/work" } });
    let written = await eventually(
      writer.payloads,
      (all) => all.filter(named("ses_fallback01")).length === 2,
      "the route and the session's state",
    );
    const mine = written.filter(named("ses_fallback01"));
    assert.deepEqual(
      mine.map((payload) => [payload.hook_event_name, payload.live?.state, payload.live?.detail]).sort(),
      [["seed", "waiting", "permission"], ["tui.route", undefined, undefined]],
    );
    const snapshot = mine.find((payload) => payload.live)!;
    assert.equal(snapshot.live?.pid, process.pid, "the client's own process");
    assert.equal(snapshot.attribution, undefined, "live state takes no attribution path");
    assert.equal(snapshot.cwd, "/work");

    client.push({ type: "permission.replied", data: { sessionID: "ses_fallback01", requestID: "per_t1", reply: "once" }, location: { directory: "/work" } });
    written = await eventually(writer.payloads, (all) => all.filter(named("ses_fallback01")).length === 3, "the answer");
    assert.deepEqual(
      written.filter(named("ses_fallback01")).filter((payload) => payload.live).map((payload) => payload.live?.state),
      ["waiting", "working"],
    );
    assert.ok(!JSON.stringify(written).includes("SENTINEL"));
  } finally {
    cleanup();
  }
  await new Promise((resolve) => setTimeout(resolve, 200));
  assert.equal(writer.payloads().filter(named("ses_fallback01")).length, 3, "closing the client ends nothing");
});

test("the TUI entrypoint sends no live state while the server entrypoint reports", { skip }, async () => {
  const { default: plugin } = await import("../src/v2/tui.ts");
  const client = standInClient(() => ({ location: { directory: "/work" }, data: [{ id: "conspectus-hook", features: { server: true, tui: true }, state: { status: "active" } }] }));
  client.permissions.ses_withserver = [ask("per_t2", "ses_withserver")];
  const cleanup = (await plugin.setup(client.ctx as never)) as () => void;
  try {
    await eventually(() => client.asked, (asked) => asked.length === 1, "the plugin list");
    client.show("ses_withserver");
    client.push({ type: "session.status", data: { sessionID: "ses_withserver", status: { type: "busy" } }, location: { directory: "/work" } });
    client.push({ type: "permission.asked", data: { id: "per_t3", sessionID: "ses_withserver" }, location: { directory: "/work" } });
    const written = await eventually(writer.payloads, (all) => all.filter(named("ses_withserver")).length >= 2, "the attribution records");
    await new Promise((resolve) => setTimeout(resolve, 200));
    const mine = writer.payloads().filter(named("ses_withserver"));
    assert.ok(written.length > 0);
    assert.deepEqual(
      mine.map((payload) => [payload.hook_event_name, payload.live]).sort(),
      [["session.status", undefined], ["tui.route", undefined]],
      "attribution only",
    );
  } finally {
    cleanup();
  }
});
