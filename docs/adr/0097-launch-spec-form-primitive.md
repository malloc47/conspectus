# ADR 0097: Shared Launch-Spec Form Primitive

## Status

Accepted

## Context

ADR 0096 introduces ephemeral harness launch (`conspectus mux
launch`). Its TUI form and the existing pin-create form
(`PinCreateState`, `src/tui/widgets/pins.rs`) overlap on every
field that describes *what to launch*:

- harness key (`TextInputState` today, with a `known_harness_keys`
  autocomplete list)
- cwd (`PathOmniboxState` with `known_cwd_candidates`)
- mux name (`TextInputState` with `known_mux_names` /
  `known_pin_mux_names` collision detection)
- mux socket (`TextInputState`, optional)
- launch argv override (`TextInputState`, harness-adapter default
  when empty)
- worktree toggle + branch (`worktree_enabled`,
  `worktree_branch: TextInputState`)
- transient error banner (`error: Option<String>`)

`PinCreateState` also carries pin-specific fields:

- create mode (`NewVariation` vs `AdoptSelected`) and the adopt
  provenance (`adopt_defaults`, `adopt_source_mux_name`,
  `adopt_auto_uncheck_armed`, `adopt_auto_checked_by_collision`)
- pin id, display name, and the "operator has overridden the
  auto-derived value" flags (`id_overridden`, `display_overridden`,
  `mux_overridden`)
- pin write store (`PinCreateStore::{Auto, Project, User}`)
- known pin ids / mux names for collision checks
  (`known_pin_ids`, `selected_pin_id`, `known_pin_mux_names`)

The operator preference recorded during ADR 0096 intake was
explicit: "the TUI dialog should not be tied to either concept and,
instead, be parameterizable so it can be launched to satisfy either
need when summoned … the dialog should make it clear what will
happen, but the behavior should be defined by how it is launched,
not by something toggle-able to change behavior midway through."

Two shapes satisfy that constraint:

1. **Two independent overlays** with a shared low-level field set —
   `PinCreateState` and a new `MuxLaunchState`, each owning its
   own `TextInputState` instances, cursor logic, and dispatch.
   Cheap to land but duplicates the field-derivation, autocomplete,
   collision-detection, and rendering logic that already lives in
   `PinCreateState`. Every future field addition (e.g. `launch.env`)
   costs two edits.
2. **A shared launch-spec form primitive** owning the common
   fields and rendering, wrapped by thin per-caller states that
   add caller-specific fields and dispatch. More refactor cost up
   front; every future field addition is one edit.

Shape (2) is the operator's stated intent — a parameterized
primitive summonable from either concept. It also aligns with the
ADR 0085 MVU contract: the reducer's mutation intent stays with the
per-caller `PinCreateRequest` / `MuxLaunchRequest`; only the *form
state* is shared.

The question this ADR answers: **what is the shape of the shared
form primitive, how are the two callers wired to it, and how does
the refactor land without regressing the ADR 0094 worktree toggle
or the ADR 0057 adopt flow?**

## Decision

Introduce `LaunchSpecFormState` — a shared TUI form primitive that
owns the fields common to pin-create and mux-launch. Wrap it with
per-caller states (`PinCreateState`, `MuxLaunchState`) that add
caller-specific fields, own their own cursor sequences, and emit
their own commit `Msg`.

### Field ownership

`LaunchSpecFormState` owns:

- `harness: TextInputState`
- `cwd: PathOmniboxState`
- `mux_name: TextInputState`
- `mux_socket: TextInputState`
- `launch_argv: TextInputState`
- `worktree_enabled: bool`
- `worktree_branch: TextInputState`
- `error: Option<String>`
- `known_harness_keys: Vec<String>` (for the harness autocomplete)
- `known_mux_names: Vec<String>` (for mux collision detection)

`PinCreateState` wraps `LaunchSpecFormState` and adds:

- `mode: PinCreateMode` (`NewVariation` / `AdoptSelected`)
- `adopt_defaults: Option<PinCreateDefaults>`, plus the adopt
  auto-toggle state (`adopt_auto_uncheck_armed`,
  `adopt_auto_checked_by_collision`)
- `id: TextInputState`, `display_name: TextInputState`
- `id_overridden: bool`, `display_overridden: bool`,
  `mux_overridden: bool`
- `store: PinCreateStore`
- `known_pin_ids: Vec<String>`, `selected_pin_id: Option<String>`,
  `known_pin_mux_names: Vec<String>`
- `name: TextInputState` (the legacy "friendly name" field —
  retained until its consumers migrate)

`MuxLaunchState` wraps `LaunchSpecFormState` and adds:

- `scan_root_hint: Option<String>` (passed through to the
  subprocess for consistency with pin launch; harmless when unset)

Nothing else. The mux-launch caller has no id, no display name, no
store, no adopt provenance. The primitive alone plus the
scan-root hint suffices.

### Parameterization

Each caller sets its parameters at open time. The form does not
inspect them mid-flow:

- **Title** (rendered at the popup's top): `"Create pin"` /
  `"Adopt selected mux as pin"` / `"Launch harness in new mux — no
  pin"`. The wrappers own the title string; `LaunchSpecFormWidget`
  takes it as a render argument.
- **Visible field set**: the wrapper decides which sequence of
  fields the cursor traverses and which the widget renders.
  `LaunchSpecFormState` exposes each shared field as an addressable
  cell; the wrapper's cursor enum names the ordered subset.
- **Submit label**: `"Create"` / `"Adopt"` / `"Launch"`. Wrapper-
  owned, rendered in the footer hint.
- **Commit `Msg`**: `Msg::CommitPinCreate(PinCreateRequest)` for
  pin-create, `Msg::CommitMuxLaunch(MuxLaunchRequest)` for
  mux-launch. Wrappers build the request from their own state
  plus a `snapshot(&LaunchSpecFormState) -> LaunchSpecSnapshot`
  helper that returns the shared-field values.

The mode is immutable after open. There is no "flip persistence
midway" control — the operator cancels and reopens through the
other entrypoint to switch shapes.

### Cursor sequence

Each wrapper owns a `Cursor` enum listing its visible fields in
render order. `LaunchSpecFormState` does not own a cursor.
Existing pin-create field order is preserved by
`PinCreateState`'s cursor; `MuxLaunchState`'s cursor is the
shared subset (harness → cwd → mux name → mux socket → launch
argv → worktree toggle → [worktree branch]).

Key dispatch stays wrapper-owned. On each key, the wrapper:

1. Handles global chords (Esc / Tab / Ctrl-C / Enter) directly.
2. If the cursor is on a shared field, forwards the key to the
   corresponding `TextInputState` / `PathOmniboxState` on the
   primitive.
3. If the cursor is on a wrapper-specific field, handles it
   locally.
4. On commit intent, calls `snapshot()`, validates against
   wrapper-owned rules (id uniqueness, mux collision, worktree
   branch presence, etc.), and either emits the wrapper's commit
   `Msg` or stores an `error` on the primitive.

### Autocomplete and collision detection

Shared logic lives on the primitive:

- `harness` autocomplete against `known_harness_keys`.
- `mux_name` collision against `known_mux_names` (live tmuxes) —
  the primitive raises a validation flag; the wrapper decides
  whether the flag is a hard error (`mux launch` — same-name
  tmux is a conflict either way) or the ADR 0057 adopt-trigger
  (`pin create` — matching mux name arms the adopt toggle).
- `cwd` autocomplete against `known_cwd_candidates` via
  `PathOmniboxState` (already primitive-shaped).

Pin-specific collision (`known_pin_ids`, `known_pin_mux_names`)
stays on `PinCreateState`.

### Rendering

`LaunchSpecFormWidget<'a>` takes:

- `state: &'a LaunchSpecFormState`
- `theme: &'a Theme`
- `title: &'a str`
- `submit_label: &'a str`
- `visible: &'a [LaunchSpecField]` — the ordered subset the
  wrapper wants rendered
- `focus: Option<LaunchSpecField>` — the wrapper's cursor
  translated into a shared-field identity, or `None` when the
  cursor is on a wrapper-specific field
- `extra_top: &'a [Line]` and `extra_bottom: &'a [Line]` — pre-
  rendered wrapper-specific chrome (adopt banner, id/display
  fields, store selector, "no pin will be written" hint, etc.)

The widget lays out title → `extra_top` → visible shared fields
(each as a `label: value` row with the focused row highlighted) →
`extra_bottom` → footer hint (`Tab · Enter · Esc · <submit_label>`).

Wrappers own their extra rows because pin-create's id/display/store
rows are visually intermingled with the shared rows in the current
layout; refactoring the *visual* order is out of scope. This ADR
sets up the state split; the widget contract admits interleaving
via `extra_top` / `extra_bottom` slices.

### Migration plan

The refactor is staged so each step keeps `cargo test --all-targets
--all-features` green. Implementation shipped in two commits:
`H-MUX-LAUNCH-001` introduced `LaunchSpecFormState` and wired
`MuxLaunchFormState` onto it as its first consumer, along with the
Mux action menu, CLI verb, and runtime executor;
`H-MUX-LAUNCH-002` then migrated `PinCreateState` onto the same
primitive with both consumers already in tree. Step 4 (rehoming
accessors + validation onto the primitive) and step 5 (the
`MuxLaunchFormState` → `MuxLaunchState` rename) were folded into
the extraction — the wrapper diet turned out thin enough that a
rename would only touch spelling, and both wrappers reach shared
fields exclusively through `spec()` / `spec_mut()`.

1. **Extract the struct.** Introduce `LaunchSpecFormState` with
   the shared fields listed above. Do not remove the shadow copies
   from `PinCreateState` yet.
2. **Delegate reads.** Add `PinCreateState::spec() -> &LaunchSpecFormState`
   and `spec_mut()`. Change every reader inside `pins.rs` and its
   tests that touches the shared fields to route through `spec()`.
   The shadow copies remain writable via `spec_mut()`.
3. **Collapse writes.** Remove the shadow copies. Every write path
   goes through `spec_mut()`. Regression baseline: the
   pin-create-form and adopt tests in `pins_tests.rs` all pass.
4. **Introduce `MuxLaunchState`.** New wrapper, own cursor,
   `snapshot()` + validation. Add `NewMuxLaunchFormWidget` that
   composes `LaunchSpecFormWidget` with mux-launch-specific
   chrome. The ADR 0095 `NewMuxFormState` overlay stays as the
   bare-mux path; `MuxLaunchState` is the harness-launch peer.
5. **Wire the runtime.** Add `Msg::CommitMuxLaunch(MuxLaunchRequest)`
   and `ExecSpec::MuxLaunch { … }` mirroring `MuxNew`; the
   executor forks into the ADR 0096 launch path.
6. **Ship the Mux action menu** per ADR 0096 (`m`-keyed overlay
   surfacing New / Launch / Attach). The new form opens from the
   menu's "Launch harness in new mux…" entry.

Each numbered step is a reviewable commit. Step 3 is the load-
bearing refactor; steps 4–6 are additive and behind ADR 0096's
new verb.

### Non-goals

- **No shared widget for pin-create's id / display / store rows.**
  Those are pin-specific and stay on `PinCreateState`. The
  primitive is scoped to the launch-spec fields only.
- **No visual redesign.** The refactor preserves the current pin-
  create field order and rendering pixel-for-pixel (as far as
  snapshot tests catch it). Any visual change is a separate
  story.
- **No commit-Msg abstraction.** Each wrapper emits its own typed
  `Msg` variant. There is no `Msg::CommitLaunchSpec(_)` shared
  variant — the request shapes diverge (persistence store, adopt
  provenance) and shared dispatch would hide that.
- **No third caller in this ADR.** The primitive is designed so a
  future caller (e.g. `pin edit`, `pin rebind`, or a hypothetical
  workspace-level batch launch) can wrap it, but those wrappers
  land under their own stories.
- **No behavior change for existing pin-create flows.** All ADR
  0057 (create), ADR 0094 (worktree toggle), and adopt paths
  keep their current UX. Regression tests are the gate.

## Consequences

- New feature work that touches launch-spec fields (e.g. adding
  `launch.env`, ADR 0057 open question deferred to a follow-up)
  edits one place instead of two.
- `PinCreateState` gets smaller — the shared fields move to a
  cleanly bounded neighbor. Pin-specific state (adopt provenance,
  id/display override flags, store selection) becomes easier to
  read because it is no longer interleaved with generic launch
  fields.
- `MuxLaunchState` lands as a thin wrapper — the ADR 0096 form
  does not duplicate autocomplete, collision detection, or field
  rendering. Its own footprint is the cursor enum, the snapshot
  translator, and the request-building glue.
- Every write path in the launch-spec form flows through
  `spec_mut()`. Grepping `spec_mut(` gives a bounded review
  surface for future mutations, matching the ADR 0087 write-path
  review discipline.
- The `NewMuxFormState` overlay from ADR 0095 stays as-is (bare
  mux, two-field form). This ADR does not fold it into the
  primitive — bare mux has only two fields, none of them
  harness-related, and the shared machinery would be pure
  overhead. Future work may unify if a third bare-shape emerges.
- Snapshot tests (`snapshot-fixture-mode`, ADR 0068) protect the
  pin-create rendering across the refactor. Any pixel-level
  divergence blocks the step-3 commit.

## Alternatives Considered

- **Two independent forms, no shared primitive.** Rejected on the
  duplication cost. Every launch-spec field is a two-edit change
  forever, and drift between the two callers becomes a review
  concern for every field addition.
- **Trait-based dispatch (`LaunchSpecForm` trait each wrapper
  implements).** Rejected as heavier than needed. The two
  wrappers share *state* and *rendering*, not behavior — a trait
  would mostly re-declare associated types without moving
  behavior into shared code.
- **Fold `NewMuxFormState` (ADR 0095 bare mux) into the primitive
  too.** Rejected. Bare mux has two fields (name, cwd), no
  harness, no argv, no worktree. Composing it against the shared
  primitive would drag in setup for fields it does not render.
- **Shared commit `Msg` with an enum payload.** Rejected. The
  request types (`PinCreateRequest`, `MuxLaunchRequest`) have
  different persistence semantics and different executor
  branches; a shared `Msg` would obscure that at the reducer
  boundary.
- **Refactor now, ship mux-launch later.** Rejected. Landing
  step 6 (menu + wiring) is the operator-visible payoff; landing
  steps 1–5 with no consumer risks bit-rot. The staged plan
  above lets each step be reviewable but keeps the whole
  sequence on one story arc.
- **Refactor pin-create's visual layout as part of the same
  story.** Rejected as scope creep. The visual chrome
  (interleaved wrapper rows vs. shared rows) is preserved by the
  `extra_top` / `extra_bottom` widget slices. A visual redesign
  can follow independently.

## Open Questions Answered

- **Does the form know its mode at open time?** Yes. Mode is
  fixed by the caller (`PinCreateState::new` vs
  `MuxLaunchState::new`) and does not change until close. No
  in-flow toggle exists.
- **Do the wrappers share commit dispatch?** No. Each wrapper
  emits its own typed `Msg` variant.
- **Are shared fields writable from both wrappers?** Yes, through
  `spec_mut()`. The wrapper's cursor decides which shared field
  the keypress lands on.
- **Does the widget know about wrapper-specific rows?** Only via
  the `extra_top` / `extra_bottom` line slices. It does not
  branch on caller identity.
- **Where does adopt live?** On `PinCreateState`. Mux launch has
  no adopt shape.
- **Where does worktree live?** On the primitive. Both wrappers
  render the worktree toggle + branch (per ADR 0096 operator
  intake).

## Open Questions Deferred

- **A `pin edit` / `pin rebind` migration to the primitive.** The
  primitive is designed to support additional wrappers, but
  edit's field set is a strict subset of create's and rebind is
  mux-name-only; the payoff is small until a fourth caller
  appears.
- **A shared "field defaults derivation" helper** that both
  wrappers call to seed initial values from the selected row.
  Today the runtime builds `PinCreateDefaults`; a future story
  may generalize to `LaunchSpecDefaults`. Deferred until the mux-
  launch defaults settle.
- **Visual redesign** to move pin-create's id/display/store rows
  into a consistent position relative to the shared rows.
  Deferred; the current interleaving is fine until an operator-
  reported ergonomic issue surfaces.
- **A commit-time "no pin will be written" confirmation on the
  mux-launch form.** Deferred until user testing indicates the
  title + submit label are insufficient to communicate intent.

## Related ADRs

- ADR 0030 (TUI text input primitive) — the `TextInputState` /
  `PathOmniboxState` primitives this ADR composes.
- ADR 0057 (session pins) — pin-create's persistence and adopt
  semantics that stay on `PinCreateState`.
- ADR 0068 (interactive fixture / snapshot mode) — the regression
  surface protecting pin-create rendering across the refactor.
- ADR 0085 (TUI MVU architecture) — the Msg / Effect / Executor
  contract each wrapper honors.
- ADR 0094 (worktree-backed pins realized at launch) — the
  worktree toggle / branch fields shared by both wrappers.
- ADR 0095 (bare mux session creation) — `NewMuxFormState`, the
  two-field bare-mux overlay this primitive deliberately does
  not absorb.
- ADR 0096 (mux launch — harness in a new mux without a pin) —
  the second wrapper this ADR is drafted to serve.
