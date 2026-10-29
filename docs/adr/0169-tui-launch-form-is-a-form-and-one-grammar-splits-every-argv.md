# ADR 0169: The Launch Form Is A `Form`, And One Grammar Splits Every Argv

## Status

Accepted. Supersedes
[ADR 0097](0097-launch-spec-form-primitive.md): the launch-spec form
primitive (`LaunchSpecFormState`) is gone, and the mux launch form is the
framework's `Form` (ADR 0151 named this; ADR 0168 left the launch form's
own ring and state for this story). Narrows
[ADR 0096](0096-ephemeral-harness-mux-launch.md): its TUI form is the one
described here.

## Context

The launch form (`N`, or **Launch harness in new mux…** in the mux menu)
was the last multi-field form after the new mux form moved onto
`tuikit::form::Form` (ADR 0168) that kept its own state, keys, and rows.
The inventory for the framework extraction
(`docs/plans/tui-framework/inventory/03-text-entry-and-forms.md`, sections
2.4, 2.6, and items D8, D9, and D20) found:

- **A primitive with one consumer.** ADR 0097 built `LaunchSpecFormState`
  to share the launch fields between pin create and mux launch. Pin create
  is gone (ADR 0141), so the primitive had one consumer, and its
  accessors, `spec_mut` review surface, and `LaunchSpecFormWidget` contract
  described a design that no longer existed.
- **Two rings.** `↑`/`↓` moved between rows and `Tab`/`S-Tab` between
  stops, and the socket checkbox, drawn at the end of the name row, was a
  stop only `Tab` reached.
- **Two tokenizers.** The form parsed its argv with its own strict
  tokenizer (`parse_launch_argv`), which kept a backslash inside double
  quotes before an ordinary character and refused an unclosed quote,
  while `argv::split_command_line` dropped that backslash and closed the
  quote silently. The pin edit form toggled options with the first and
  saved with the second, so `"a\b"` was `a\b` to its checkboxes and `ab`
  in the saved pin.
- **Copied argv logic.** Stepping the harness, flipping a launch option,
  carrying a typed argv across a change of harness, and resolving the
  default argv were written in both `launch_spec_form.rs` and
  `pin_edit.rs`, and the default argv had three definitions.
- **A file read on every frame.** The command row resolved the default
  argv while drawing, and with `atelier` on `PATH` that canonicalized the
  cwd and walked up it looking for an Atelier workspace twice per frame;
  the pin edit form's options row did the same.

## Decision

1. **The launch form is a `Form<Msg>`.** `MuxLaunchForm` holds a
   `tuikit::form::Form<Msg>` and keeps in step the fields that follow
   others, reading `Form::edited` after each key, as ADR 0168 describes
   for such a host. The fields, in order, with their ids as labels:
   `name` (text); `server` (a toggle, `[ ] own socket`); `socket` (text,
   shown while `server` is on); `harness` (a choice of the known harnesses
   that also takes a typed one, marked `[custom]`); `options` (a check
   group of the harness's launch options, shown for a harness that has
   some); `cwd` (a directory path); `argv` (text); `command` (a derived
   read-only line, or a warning when the harness has no default);
   `worktree` (a toggle); `branch` and `repo` (text and a directory path,
   shown while `worktree` is on); and `pin` (one of `don't pin`, `launch,
   then pin`, `pin, don't launch`). The field checks keep their reasons
   (`mux launch: mux name is required`, `… is already live; pick another
   name`, `harness is required`, `cwd is required`, `launch argv has an
   unclosed `'` quote`, `worktree branch is required`, `worktree repo is
   required`) and run top to bottom, so the name is checked before the
   harness. `on_submit` builds `MuxLaunchRequest` as before.
2. **One ring.** The socket checkbox is a row of its own under the name,
   labelled `server`, and a stop like any other: `Tab`/`↓` and `S-Tab`/`↑`
   visit the same stops. Its field shows under it while it is on, as the
   branch and repo show under the worktree toggle, and keeps its text
   while it is off.
3. **What stays in Conspectus.** The field list, the checks' reasons, the
   request, and the argv field's helpers, now in one place
   (`tui::widgets::launch_argv`) for both forms: `DefaultArgv` (the
   default an empty argv means and where it comes from), the effective
   argv, a launch option's flags on or off, the rule that carries a typed
   argv to a new harness (every known option's flags come off, and an argv
   that was only the old default follows the new one), and one quoter
   (`argv::shell_quote`). The pin edit form uses them and steps its harness
   with the framework's `Choice`; it moves onto `Form` itself later.
4. **The default argv is decided, then kept.** Each form decides it when
   it opens and again only when the harness or the cwd changes
   (`DefaultArgv::refresh` compares them); drawing reads the kept answer,
   so neither form reads the file system while drawing.
5. **One grammar splits every argv.** `src/argv.rs` holds the grammar and
   documents it: a POSIX shell's word splitting with nothing expanded, in
   which single quotes keep everything, double quotes keep everything but
   a backslash before `"`, `\`, `$`, or a backtick, an unquoted backslash
   makes the next character plain, and adjacent pieces join into one
   argument. `parse_command_line` refuses text that ends inside a quote or
   on a lone backslash; it reads an argv someone typed (the launch form's
   check, the pin edit form's save and `pin edit --set argv=…` through
   `pin_ops::parse_override`). `split_command_line` reads the same grammar
   as far as the text goes; it reads text Conspectus did not type (a tmux
   pane's start command, history, previews, the rebuild plan) and what an
   operator is still typing, so an option or a harness change works on the
   argv so far and writes it back finished.
6. **The framework is unchanged.** Every capability the form needed was
   already in `tuikit::form`: conditions (`visible_if`), derived and
   warning lines, one-of-N rows, a choice with a typed value, check
   groups, and `Form::edited`.

## Consequences

- **Visible changes** (the story's final summary and the changelog list
  them): the form gains a `server` row under the name and loses the
  checkbox at the end of the name row, so it is a row taller and the
  name gets the whole row; `↓` from the name goes to that row (it went to
  the harness); a refused launch moves the cursor to the field it is
  about; the error line clears on the next edit, not on a key that only
  moves the cursor; with an unclosed quote in the argv, toggling a launch
  option or stepping the harness closes it (it did nothing); and a typed
  argv the forms write back quotes `@`, `%`, and `,` no more than the
  previews do.
- **Edge-case argv text splits differently** wherever
  `split_command_line` reads it: a backslash inside double quotes before
  an ordinary character now stays (`"a\b"` is `a\b`, was `ab`), and a
  last lone backslash stands for itself (`a \` is `a`, `\`, was `a` and an
  empty argument). `pin edit --set argv=…` and the pin edit form refuse an
  unclosed quote or a trailing backslash (they closed the quote, or
  dropped the backslash), and the pin edit form's checkboxes read the
  argv the way its save does.
- A launch option row with several options would move its own cursor
  with `←`/`→` and flip with `Space`, as every check group does; no
  harness offers more than one today. The pin edit form keeps its own
  option rule until it moves onto `Form`.
- A new launch field is one `Field` in `MuxLaunchForm::new` and, if the
  request carries it, one line in `request`.

## Alternatives Considered

- **Keep `LaunchSpecFormState` as the model and draw through `Form`.** Two
  copies of every value, kept in step by hand; its premise, a second
  consumer, is gone. Rejected.
- **An `on_change` closure on `Form` instead of a wrapper.** It would move
  the follow-up logic into the form, but the closure is shared (`Fn`,
  `Send + Sync`) and cannot keep the decided default without interior
  mutability, and ADR 0168 already names the wrapper as how a host keeps
  fields in step. The seam stays open on the control page.
- **Keep the checkbox on the name row.** It needs two controls on one row
  of the ring, which the framework leaves as a seam, and house rule 6
  wants one stop per row. Rejected.
- **Label the checkbox `socket` and the field `socket name`.** Two rows
  starting with the same word, and a label longer than the short forms
  of house rule 17. `server` says what the checkbox picks.
- **Make `split_command_line` strict for every caller.** A preview of a
  tmux start command or a history record must show something for text
  Conspectus did not write. Rejected; the two readers share the grammar
  and differ only on unfinished text.
- **Keep the lenient reading for `pin edit --set`.** It silently closes a
  quote the operator forgot. Rejected.
- **Keep `split_command_line`'s old double-quote rule.** It is not what a
  shell does, so an argv shown in a preview or saved in a pin could launch
  differently than the same text typed at a shell. Rejected.
- **Memoize the workspace lookup inside `LaunchDefaults`.** A cache keyed
  by path, shared by every caller, with its own staleness rule; the forms'
  own answer, kept until their inputs change, is local and plain.

## Open Questions Answered

- *Is the launch form a `Form`?* Yes, with a host wrapper for the fields
  that follow others.
- *Where does the socket checkbox go?* A `server` row of its own, under
  the name, on the one ring.
- *Which tokenizer?* One grammar in `src/argv.rs`, strict for an argv
  someone types and as-far-as-it-goes for text read back.
- *When is the default command decided?* When the form opens and when the
  harness or the cwd changes; never while drawing.
