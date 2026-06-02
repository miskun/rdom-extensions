# rdom-extensions Agent Guide

This file defines how AI agents work in this repository.

`rdom-extensions` provides optional **data-visualization components** (charts, sparklines, gauges,
virtual tables) for [rdom](https://github.com/miskun/rdom). It is a **downstream consumer** of the
rdom substrate, deliberately kept out of the rdom workspace because rdom ships zero opinionated
components by design.

## Where to look first

- `STATE.md` — milestone plan, decisions, open risks. Read it before starting work.
- `README.md` — what's shipped vs planned.
- The rdom repo at `../rdom` — `specs/DESIGN.md` (substrate-first rationale), `CLAUDE.md` (the
  parent project's engineering rules, which apply here too).

## Non-negotiable principles

- **Public API only.** Build strictly on `rdom-tui`'s public surface: the `<canvas>` paint API
  (`canvas::set_paint` + `RenderContext`), element builders, the cascade, and runtime event
  listeners. Never reach into rdom internals or fork rdom code. If something needs a new hook,
  that's a change request against rdom, not a workaround here.
- **Port faithfully.** Components originate in `../lens-k8s-tui` (ratatui-based). Port the
  algorithms (braille rasterizer, data buffers, axis math); rewrite the rendering layer against
  `RenderContext` and swap `ColorToken`/`Theme` for `rdom_tui::Color`/`Style`. Don't reinvent.
- **One component = one element + a paint closure.** Charts mount as a `<canvas>` with a
  `set_paint` callback; state lives behind `Rc<RefCell<…>>` (`*View` handles) so the closure
  borrows it and the app mutates it between frames.

## Engineering rules (inherited from rdom)

- **TDD always** — failing test first, then the smallest change to pass. Pure logic gets unit
  tests; rendering gets headless integration tests (cascade → `layout_dom` → `Buffer::empty` →
  `paint_dom` → inspect cells). Docs-only changes are the exception.
- **Real fixes only** — no `#[allow(dead_code)]` to dodge the gate, no silent fallbacks. A
  foundation with no consumer trips `clippy -D warnings`; land it with its first consumer.
- **Code and docs move together** — update `STATE.md` in the same commit as a meaningful decision.

## The gate (must pass before every commit destined for push)

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Doc-only commits skip the test pass. If a gate command fails, fix and re-run before committing —
no `fix: drop unused …` follow-ups.

## Milestone review gates

At the end of each milestone, run the two review passes from the rdom working agreement (Grumpy
Chief Architect + Grumpy Chief Product/API) and record findings in `STATE.md` before starting the
next milestone.
