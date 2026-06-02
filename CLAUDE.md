# rdom-charts Agent Guide

This file defines how AI agents (Claude Code, Codex, Cursor, Aider, and anything honoring the
[agents.md](https://agents.md) convention) work in this repository. `AGENTS.md` at the repo root is
a symlink to this file — edit `CLAUDE.md`; `AGENTS.md` follows automatically.

`rdom-charts` provides terminal **chart components** (time-series, sparkline, bar, gauge) for
[rdom](https://github.com/miskun/rdom), the browser-faithful DOM for terminal applications. It is a
**downstream consumer** of the rdom substrate, deliberately kept out of the rdom workspace because
rdom ships native HTML elements and *zero opinionated components* by design. (The virtualized table
lives in the sibling `rdom-virtualtable` crate — a different mechanism, its own repo.)

Keep this file current. If the project makes a durable process, architecture, or quality decision,
update `CLAUDE.md` in the same change.

## Where to look first

- `STATE.md` — the living project journal: milestone plan, decisions, open risks. Read it before
  starting work, and update it as you go.
- `README.md` — what's shipped vs planned (keep the status table honest).
- The rdom repo at `../rdom` — `specs/DESIGN.md` (the substrate-first rationale and the public API
  this crate consumes) and its `CLAUDE.md` (the parent working agreement these rules mirror).
- The code itself: each module has a top-level doc comment, and tests document the contracts.

## Non-Negotiable Project Principles

- **Public API only.** Build strictly on `rdom-tui`'s published public surface: the `<canvas>`
  paint API (`canvas::set_paint` + `RenderContext`), element + text builders, the cascade, and
  runtime event listeners. Never reach into rdom internals, never fork rdom code, never add a path
  dependency back into the rdom source tree (we depend on the crates.io release `rdom-tui = "0.3"`).
  If a component genuinely needs a new hook, that is a change request against rdom — not a
  workaround here.
- **Theme-agnostic, math separate from paint.** Components speak `rdom_tui::Color`/`Style`
  directly — never an app-specific color-token or theme abstraction. Keep the math (braille/block
  rasterizers, axis/tick computation, data buffers, EMA) independent of the paint layer so it's
  unit-testable without a terminal; the thin paint step draws through rdom-tui's `RenderContext`.
- **One component, one pattern.** A chart is a pure logic type (state + `paint(&RenderContext)`)
  plus a `*View` handle (`Rc<RefCell<…>>`) exposing `mount(dom) -> NodeId` and `with(|c| …)`. The
  canvas paints via `set_paint`; state lives behind the `Rc<RefCell>` so the paint closure borrows
  it and the app mutates it between frames, then requests a repaint (`ctx.request_redraw()`).
- **No opinionated frameworks.** This crate ships composable visualization primitives, not an
  application framework. Higher-level dashboards/app shells belong in *their* downstream projects.

## Engineering Principles

### TDD Always

Write tests before implementation.

1. Add or update a failing test that describes the desired behavior.
2. Run the smallest relevant test command and confirm the failure.
3. Implement the smallest change that makes the test pass.
4. Run the relevant tests again.
5. Refactor only after tests are green.

Two test seams in this crate:
- **Pure logic** (scaling, windowing, tick math, zone selection, layout partitioning) → ordinary
  unit tests, no terminal. Factor the math into a pure method so it is testable in isolation.
- **Rendering** → headless integration tests that drive the real pipeline:
  `cascade → layout_dom → Buffer::empty → paint_dom`, then inspect cells (assert braille/block
  glyphs or text land where expected). See `tests/render_*.rs` for the idiom.

Docs-only changes are the exception.

### Real Fixes Only

- Reproduce a bug with a failing test, fix the root cause, keep regression coverage.
- No `#[allow(dead_code)]` (or other lint-silencing) to dodge the gate. A foundation with no
  consumer trips `clippy -D warnings` for dead code — that is the gate telling you to land it
  *with* its first consumer, not to suppress it.
- No silent fallbacks that hide a NaN, an empty dataset, or a zero-sized canvas — handle them
  explicitly (empty state, clamped range) and test that path.
- Don't duplicate a rasterizer/formatter across components; share it (`chart::braille`,
  `chart::blocks`, `chart::axis`).

### Contract First

Public behavior is a contract before implementation:
- Domain types + unit tests describe a component's behavior before the paint code lands.
- The public surface is the `lib.rs` / module `pub use` re-exports — nothing user-facing leaks by
  `pub(crate)` accident, and nothing is exported that has no effect yet (don't advertise unshipped
  behavior; e.g. a stacking mode with no implementation stays unexported until it works).

### Architecture Hygiene

Keep modules small and single-purpose. Watch for: a `*Chart`/`*Table` type absorbing unrelated
responsibilities; mixing data model, scaling, and paint in one blob; duplicated rasterization;
hidden global state. Prefer small types, explicit `*View` handles, and narrow modules. Split early
rather than late.

### Charting Fidelity

Where a web/charting convention exists, follow it: nice-tick axis selection, min-at-bottom /
max-at-top orientation, gaps for `NaN`, sub-cell rasterization for smooth lines. Document any
deliberate departure at the API and in `STATE.md`.

### Safety By Default

- Bounds-check every sub-cell write (the braille/block helpers and `RenderContext` already clip;
  keep it that way).
- Free DOM nodes you stop using (`drop_subtree`) so re-materialization doesn't leak arena slots.
- Saturating arithmetic on widths/offsets — never let a layout underflow panic.
- Test the empty/degenerate/out-of-range paths, not just the happy path.

### Quality Bar

Aim for ultra performance, beautiful output, decoupling, and modular architecture. Boring, correct,
testable, explicit beats clever.

## Testing Commands

Run the smallest relevant command first, then broaden before finishing.

```bash
cargo test                       # all unit + integration tests
cargo test --lib                 # unit tests only (pure logic)
cargo test --test render_time_series   # one integration file
```

Workspace gate (the same set every commit destined for push must pass):

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

If a command cannot run because the environment is wrong (e.g. no network to fetch the published
`rdom-tui`), say so clearly in the final response — do not commit.

## Commit Discipline

Commit after each completed implementation or documentation step once the relevant checks pass.

- Keep commits scoped to the completed step; don't leave finished green work uncommitted.
- Do not commit if relevant tests are failing.
- Do not mix unrelated dirty-worktree changes into a commit.
- End commit messages with the `Co-Authored-By` trailer used across these projects.

### Pre-commit hygiene gate (mandatory)

Before every commit destined for push, the three-command gate (`cargo fmt --check` /
`cargo clippy --all-targets -- -D warnings` / `cargo test`) must pass clean. Doc-only commits skip
the test pass.

**Rule:** if any gate command fails, fix and re-run before committing. Do not ship `fix: drop
unused …` follow-up commits — those are evidence the gate was skipped.

### Clean-entry-point rule after push

After every `git push`, the local repo must be in a state where `/clear` is safe — working tree
clean, branch synced, no half-built artifacts. If a clean entry point isn't reachable this turn,
say so explicitly and let the user decide.

## Progress Tracking

`STATE.md` is the living project journal. Update it whenever a change makes or records a meaningful
decision: completed milestones, current status, ad-hoc product/technical decisions, review
findings, open risks, follow-up tasks, and *why* the project moved a particular way. Keep it useful,
current, and honest — not a marketing doc.

## Milestone Review Gates

At the end of every milestone, run two review passes before starting the next, and record the
findings in `STATE.md`.

### Grumpy Chief Architect Pass

Review for: correctness and root-cause quality; performance risks (allocation in the paint path,
redundant work); coupling and modularity; god objects or oversized files; duplicated logic; weak
contracts; missing tests, especially error/empty/degenerate paths; hidden operational risk (arena
leaks, panics on bad input). Output: what's strong, what to improve, blocking findings, non-blocking
findings, required follow-ups.

### Grumpy Chief Product/API Pass

Review for: whether the milestone is something a real consumer can build on without surprise;
whether the public API is honest (no exported no-ops, no oversold features); whether the behavior is
explainable; whether examples/tests still demonstrate what they claim. Output: same shape as above.

### Gate Rule

Do not start the next milestone until key findings are addressed or explicitly recorded in
`STATE.md` as accepted risks / scoped-out work.

## Agent Workflow

Before editing: read the relevant module + tests and `STATE.md`; identify the smallest behavior
change; add or update tests first unless docs-only.

While editing: keep changes scoped; prefer existing patterns (pure-logic + `*View`); keep the public
surface (`pub use`) synchronized; depend only on rdom-tui's public API.

Before final response: run the relevant tests (then the full gate for non-trivial changes); report
what changed and the verification results; call out anything not run.
