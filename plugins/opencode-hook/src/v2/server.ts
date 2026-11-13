// Conspectus opencode hook plugin for opencode 2.x (server entrypoint).
//
// opencode 2.x runs sessions inside one shared service, and a server
// plugin is instantiated there once per location it serves. Only the
// service sees every session, including those no client shows (a
// background subagent, a turn that goes on after its client closed), so
// live state comes from here: the plugin hands the location's session
// events to the live-state reducer (`../live.ts`), which sends each
// session's state to `conspectus hook write opencode` whenever it changes
// (ADR 0206 decision 3, amending ADR 0126 decision 3).
//
// The service's environment belongs to whichever client started it, so
// the writer marks these records as coming from a background harness and
// records no pane for them (ADR 0126 decision 1); nothing here is ever
// recorded for attribution. Which session a pane shows still comes from
// the TUI entrypoint (`tui.ts`), which runs in each client.

import type { Plugin } from "@opencode/plugin";
import { V2_EVENTS, createLiveState, newInstanceId } from "../live.js";
import { driveLiveState, ownStartTime } from "../write.js";

// Typed against the package's `Plugin` interface rather than built with
// `Plugin.define` (an identity function), so the plugin has no runtime
// dependency to resolve.
const plugin: Plugin.Plugin = {
  id: "conspectus-hook",
  setup(ctx) {
    const live = driveLiveState(
      createLiveState({
        instance: newInstanceId(),
        pid: process.pid,
        pidStart: ownStartTime(),
        directory: ctx.location.directory,
      }),
    );
    const stop = new AbortController();
    void (async () => {
      try {
        // opencode checks the names at run time: one a version lacks is
        // ignored, with a warning in the service log.
        const events = ctx.event.subscribe({ types: [...V2_EVENTS] as never, signal: stop.signal });
        for await (const event of events) {
          live.send((state, now) => state.v2(event, now));
        }
      } catch {
        // The stream ended or failed. There is nothing to report, and
        // opencode must not notice.
      }
    })();
    return () => {
      stop.abort();
      live.dispose();
    };
  },
};

export default plugin;
