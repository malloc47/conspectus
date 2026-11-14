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
