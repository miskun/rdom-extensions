# rdom-charts — Project State

Living journal for the terminal charts crate built on top of rdom.

> **Crate split (2026-06-02).** This crate was originally `rdom-extensions` and also held a
> virtualized table. The table was a different mechanism (element-tree, not canvas) with its own
> roadmap, so it was extracted to the **`rdom-virtualtable`** crate (its own repo,
> `miskun/rdom-virtualtable`) and this crate was renamed **`rdom-charts`** (charts only). The
> `M5 — Virtual table` milestone and the dashboard's old "pods" panel below are pre-split history.

## Thesis

The rdom workspace ships a substrate (DOM + cascade + layout + paint + runtime) and native HTML
elements only — **zero opinionated components**, by an explicit non-negotiable rule in its
`CLAUDE.md` / `specs/DESIGN.md`. Chart components (time-series, sparkline, bar, gauge)
are opinionated components, so they live here as a **downstream consumer crate**, not in the rdom
publish set. We build strictly on `rdom-tui`'s public API (canvas paint, cascade, layout, event
listeners) and never touch rdom internals.

The chart math (braille grid + Bresenham, block fills, nice-ticks, EMA, data buffers) is kept
independent of the paint layer so it's unit-testable; the thin paint step draws through the canvas
`RenderContext`, and everything is theme-agnostic (`rdom_tui::Color`/`Style`, no app-specific theme).

## Architecture

- One component = one `<canvas>` element + a paint closure that draws via `RenderContext`.
- Charts rasterize onto a `BrailleGrid` (2×4 dots/cell), then flush into a `ctx.sub(...)` rect.
- State lives behind `Rc<RefCell<…>>` (the `*View` types) so the paint closure can borrow it and
  the app can mutate it between frames, then request a repaint.
- `rdom-tui` is a plain crates.io dependency (**`"0.3.14"`** as of 2026-06-07) — no path dep, so
  this crate builds standalone and never reaches into the rdom source tree.

## 2026-06-07 — bumped to rdom-tui 0.3.14 + pre-publish review

Bumped the dep `"0.3.5" → "0.3.14"`. Every intervening rdom release (0.3.6 … 0.3.14) was a
**non-breaking `rdom-tui`-only** change — `<table>` column-sizing fixes, half-block border welding,
`display:none`/`drop_subtree` correctness, text-selection / drag-autoscroll, plus additive
`App::advance(ms)` / `Dom::set_drag_autoscroll`. None touch the public surface this crate consumes
(canvas paint, cascade, layout, event listeners), so the bump needed **no code change**: gate clean
(fmt + `clippy -D warnings` + 80 tests), `cargo publish --dry-run` packages + verifies.

Ran the pre-publish Grumpy Chief Architect + Product/API passes. Findings actioned:
- **(fixed, blocking by our own rule)** Two `#[allow(dead_code)]` in `braille.rs` masked the
  `StackedPoint.baseline` → `b_frac` → `ScaledPoint.baseline_by` chain — always `0.0`, computed, and
  read by no renderer (scaffolding for unshipped area/stack fill). The crate's `CLAUDE.md` forbids
  `#[allow(dead_code)]` to dodge the gate ("land it with its first consumer, not suppress it"), so
  the dead fields/computation were removed. Re-add when stacked-area rendering actually lands (~4
  lines). No behavior change; all render tests still green.
- **(fixed)** `format_timestamp` could emit an invalid month `"13"` on the multi-day `MM/DD` branch
  (`(days % 365) / 30 + 1` overflows for the last 5 days of the synthetic 365-day year). Month is
  now clamped to 12; regression test `axis::format_timestamp_month_never_exceeds_twelve`. The
  formatter is still a deliberately calendar-free placeholder (documented) — real dates come from a
  caller-supplied `XAxisConfig::format`.
- **(fixed)** README install snippet pinned `rdom-tui = "0.3.4"`; bumped to `0.3.14`.
- **(accepted, non-blocking)** `paint_y_axis` takes 9 args (`#[allow(clippy::too_many_arguments)]`) —
  a legit allow, not a dead-code dodge; could fold into a small params struct later. Per-frame
  allocation in the paint path (collected/stacked/scaled/grid) remains accepted (see M2 review).

No blocking findings remain; the crate is publish-ready at `0.1.0`.

### Publish-hygiene pass (learned from the `rdom-virtualtable` 0.1.0 release)

Compared against the just-released sibling crate and adopted its release polish:
- **Lean tarball.** Added an `include = [...]` allow-list to `Cargo.toml` so the package ships only
  `src` + `examples` + `tests` + `README`/`CHANGELOG`/`LICENSE`. Before, `cargo package --list`
  bundled `STATE.md`, `CLAUDE.md`, `AGENTS.md`, `RDOM_SUBSTRATE_FINDINGS.md`, `rust-toolchain.toml`
  **and a stray `.claude/scheduled_tasks.lock`** — none belong in a published crate. Now 28 files,
  all crate/docs/examples/tests.
- **`#![deny(missing_docs)]`** in `lib.rs` + documented the full public surface (43 previously
  undocumented items: `DataPoint`/`TimeRange`/`Series`/`Guideline`/axis-config fields + ctors, the
  `Bar`/`Gauge`/`*View` builders/mount/with). `cargo doc -D warnings` is clean; docs.rs will render
  the whole API. Future public additions now fail the build until documented.
- **`CHANGELOG.md`** — Keep-a-Changelog format with a `0.1.0` entry (mirrors rdom-virtualtable).
- **README** — added crates.io / docs.rs / license badges and a "Requires Rust 1.85+ (edition
  2024)" line.

Gate still clean (fmt + clippy + 79 tests); `cargo doc -D warnings` clean; tarball verified lean.

## 2026-06-03 — bumped to rdom-tui 0.3.4 (focus vocabulary)

rdom 0.3.4 ships `FOCUS-VOCAB-1`: the UA focus tint is now scoped to interactive controls. A
focused `<canvas>` stays transparent the same as before — but now because it isn't a control, not
via the explicit 0.3.1 `canvas:focus` exemption (which 0.3.4 removed). No code change needed here
(this crate never carried a focus workaround — rdom's UA handled it); refreshed the comment on
`focused_chart_canvas_keeps_transparent_background` to reference the 0.3.4 mechanism. Bumped the dep
`"0.3" → "0.3.4"`. The chart canvas focus story is unchanged: clean by default, no workaround.

## 2026-06-02 — focus-gray fixed upstream (rdom 0.3.1)

The interactive chart came up with an ugly gray fill behind the plot. Root cause was upstream: the
UA `:focus { background !important }` indicator painted over the focused `<canvas>` (the example
focuses it so keys work immediately), and it was unoverridable. Fixed in rdom **0.3.1**
(`UA-FOCUS-OVERRIDABLE-1`): `<canvas>` is now exempt from the focus tint (web-faithful — focusing a
canvas never touches its pixels), so the chart is clean **by default with no workaround here**.
Bumped the dep to `0.3.1`; added `tests/render_interaction.rs::focused_chart_canvas_keeps_transparent_background`
pinning that a focused chart canvas computes `bg = Reset`.

## 2026-06-02 — rdom 0.3.0 payoff cashed in

The substrate findings this crate surfaced (`RDOM_SUBSTRATE_FINDINGS.md`) were fixed in rdom 0.3.0.
Bumped the dependency `0.2` → `0.3` and collected the wins:
- **Dropped the `flex()` field-poking workaround** — the examples/test now use `TuiStyle::flex_row()`
  / `flex_column()` (0.3's `TUISTYLE-FLEX-BUILDER-1`) instead of manually setting `display`/`flow`
  `Value` fields.
- **Wired real chart interaction** — `TimeSeriesView::install_interaction(dom, canvas)` attaches
  keyboard (`+`/`-` zoom, `h`/`l`/arrows pan, `0` reset), wheel-zoom, and left-drag-pan listeners.
  Each mutates the shared chart and calls `ctx.request_redraw()` (0.3's `EVENT-REDRAW-1`) — the
  thing that was *blocked* before (a canvas reading external `Rc<RefCell>` state couldn't trigger a
  repaint from an event handler). New chart methods: `pan_by_fraction`, `pan_by_columns`,
  `window_duration`. New `examples/interactive_chart.rs`. Headless test: `tests/render_interaction.rs`
  (5 tests — dispatch synthetic keys, assert window changed + `redraw_requested`).
- (`CANVAS-TEST-CTOR-1` / `RENDERCTX-DEDUP-1` also available now — not yet exercised here.)
- 80 tests green; clippy + fmt clean.

## Milestones

### M1 — Charting foundation ✅ (done)
- `palette` — `Color` palette + `series_color()` (replaces an app-specific `ColorToken`/`Theme`).
- `chart::data` — `DataPoint`, `TimeRange`, `Series`, `SeriesBuffer` (sorted/dedup/bounded/lazy).
- `chart::axis` — `nice_ticks`, `format_y_value`, `format_timestamp`.
- `chart::braille` — `BrailleGrid` + Bresenham + EMA + scale pipeline, retargeted to `RenderContext`.
- 24 unit tests, green.
- *Note:* M1 was folded into the M2 commit — a foundation with no consumer trips the
  `clippy -D warnings` dead-code gate, so it landed together with its first consumer.

### M2 — Time-series line chart ✅ (done)
- `chart::time_series` — `TimeSeriesChart` (pure state + `paint(&RenderContext)`) and
  `TimeSeriesView` (`Rc<RefCell>` handle; `mount(dom) -> NodeId`, `with(|chart| …)`).
- Full render pipeline: collect → stack → smooth → scale → render → decorate → paint,
  mapped to canvas-local coords (legend row / y-gutter / plot / x-axis row).
- Static + streaming constructors, windowing, follow, pan, zoom, EMA smoothing, guidelines,
  legend, nice-tick axes, empty state.
- Tests: 11 unit (window math, scaling) + 3 integration (real cascade→layout→paint, asserting
  braille glyphs / "No data" land on the buffer). 35 tests total, all green.
- Gate: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test` all clean.

### M3 — Sparkline ✅ (done)
- `chart::sparkline` — `Sparkline` (values + color + optional pinned range; pure `scale()` +
  `paint()`) and `SparklineView` (same `Rc<RefCell>` mount/`with` pattern). No axes/gutter/legend —
  the whole canvas is plot area; evenly-spaced values, `NaN` gaps, braille line.
- Tests: 6 unit (scale math: bounds, endpoints, min/max orientation, NaN gap, single-value center,
  range override) + 2 integration (renders braille into a tiny inline canvas; streaming update).
- Gate clean.

### M4 — Bar chart + rich gauge ✅ (done)
- `chart::blocks` — `h_bar(width, ratio)` eighth-block horizontal fill (sub-cell precision);
  pure, tested (full/empty/half/fractional/exact-width invariant).
- `chart::bar` — `BarChart` (horizontal labeled bars; auto/pinned max; optional value readout;
  per-bar or palette color; pure `layout()` partitioning) + `Bar` + `BarChartView`.
- `chart::gauge` — `Gauge` (linear gauge richer than native `<meter>`: fill colored by which
  `GaugeZone` the value lands in; optional label + readout; pure `ratio()`/`fill_color()`) +
  `GaugeZone` + `GaugeView`.
- Tests: 5 blocks + 7 bar + 9 gauge unit + 4 integration (bar blocks+labels, longer-bar-more-blocks,
  gauge fill+label+readout, gauge update). Gate clean. 66 tests total.

### M5 — Virtual table (core) ✅ (done)
Element-tree-based (not canvas), built on native `<table>`/`<thead>`/`<tbody>` + the table
builtin's column sync.
- `table::VirtualTable` — model (columns + rows) + pure `window_for(viewport_rows, scroll_y,
  total) -> (start, count)`.
- `table::VirtualTableView` — `mount(dom)` builds `<table>` with a header + empty `<tbody>`;
  `show_window(dom, start, count)` materializes **only** that row slice (drops the previous one
  via `drop_subtree`, re-syncs column widths); `with(|t| …)` updates data; `mounted_row_count()`
  for assertions.
- Tests: 5 unit (window math + model bookkeeping) + 3 integration (only the window materializes
  against a 1000-row model; show_window replaces the prior window; past-end renders header only).
  74 tests total. Gate clean.

**Scoped out of M5 (deliberate, not done):** this is the virtualization *core*, not a full-featured
virtual table. Deferred: automatic scroll → window recompute + a spacer so the scrollbar
reflects total rows; sorting; row/cell selection; column resize/reorder/hide; side-loaded data
sources; persistence callbacks. Tracked as follow-ups (candidate M7).

### M6 — Runnable examples ✅ (done); interaction ⏳
- `examples/dashboard.rs` — a static dashboard showing the chart components at once (two gauges,
  bar chart, sparkline, time-series with legend/axes). `cargo run --example dashboard`.
- `examples/live_chart.rs` — a streaming time-series driven by `App::on_tick`: each idle tick
  pushes a sample through the shared `TimeSeriesView` and calls `request_redraw`.
  `cargo run --example live_chart`.
- `tests/render_dashboard.rs` — headless twin of the dashboard build (cascade→layout→paint, dumps
  the frame with `--nocapture`, asserts every component painted). The validator for the example's
  layout.

**Key layout lesson (recorded so we don't relearn it):** flex distribution reads
`direction`/`gap`/`width`/`height` from the **computed style**, not the `ext` fields the node
setters (`set_direction`/`set_width`/…) write. So layout for composed UIs must go through
`TuiStyle` (inline style or a stylesheet): `display: flex` = `Display::Block` + `Flow::Flex` (set
the public `display`/`flow` fields), then `.direction()/.gap()/.width()/.height()/.padding()`
builders. The `ext` setters still feed accessors/other paths but are ignored by flex.

**Still pending (interaction):** keyboard zoom/pan/follow on the time-series, mouse/scroll on the
table. The blocker is ergonomic: an event listener (`TuiEventCtx`) mutating a `*View`'s external
`Rc<RefCell>` state doesn't trip the `DirtyTracker` (no DOM mutation), so it won't repaint on its
own. Options to design in M7: (a) the listener also touches a DOM attr on the canvas to force a
repaint; (b) add a small `*View` helper that does that; (c) request an `AppHandle`-style redraw
hook usable from `TuiEventCtx`. Until then, charts update via `view.with(...)` + a tick/redraw
(as `live_chart` shows).

## Substrate findings (for the rdom side)

`RDOM_SUBSTRATE_FINDINGS.md` collects friction / web-platform divergences / workarounds hit while
building on `rdom-tui 0.2`, for the rdom maintainer to triage. Top two: **(1)** geometry node
setters (`set_width`/`set_direction`/…) are silently ignored by flex layout — layout must go through
`TuiStyle`; **(4)** event listeners can't request a repaint when paint inputs live outside the DOM,
which blocks interactive canvas components (M7).

## 2026-06-02 — time-series gallery: 7 navigable time-series demos

Built a navigable gallery of seven time-series demos (Smooth, Spiky, Single, Dense, Live, Live Spotty, Empty)
in `examples/timeseries_gallery.rs` — a navigable gallery (keys `1`-`7`), with the two Live demos
streaming on `App::on_tick` and root-level interaction (zoom/pan/wheel/drag) operating on the
current view so a canvas swap never loses a listener.

**Feature audit: complete for the demos.** Everything the storybook uses was already
present — static + streaming constructors, EMA smoothing, Y-axis min/max/`format`, follow mode,
`ConnectPolicy::Gap`, and **guidelines/threshold lines** (`set_guidelines` + `Guideline`). The
`100ms`/`150ms` reference lines the user flagged as "missing" already worked; they just weren't
demoed. Now demoed (Spiky) + pinned headless in `tests/render_ts_demos.rs` (asserts the `100ms` /
`150ms` / `80%` labels render). 85 tests green.

## Remaining / not-yet-done (honest todo)

- **Time-series "future" styles** (marked `// Future` in `data.rs`):
  `SeriesStyle::Area` / `StepLine` (area fill, step charts), `ConnectPolicy::Connect` / `Zero`
  (bridge gaps instead of breaking), `StackMode` (stacked / percent). Optional enhancements; no
  consumer needs them yet.
- **Virtual table rich features** — moved out of this crate; tracked in the `rdom-virtualtable`
  crate (sorting, selection, column resize/reorder, scrollbar spacer + auto scroll→window, etc.).
- **Other component types not built:** `KeyValueList`, `Markdown` (would each be their own crate if
  ever wanted — out of scope; this crate is charts only).
- **Minor API:** `add_series` requires an explicit `Color` while `Series::line` auto-assigns from
  the palette — add an auto-color streaming helper for symmetry.

## Resolved (was "open questions")

- **Publish form:** crates.io `rdom-tui = "0.3.4"`, no path dep.
- **Repaint signaling:** resolved in rdom 0.3.0 (`EventCtx::request_redraw`) + wired into the
  components' interaction.
- **Runnable examples:** shipped — `dashboard`, `live_chart`, `interactive_chart`,
  `timeseries_gallery`.

## Review gates

Per the rdom working agreement, run the Grumpy Chief Architect + Grumpy Chief Product/API passes at
the end of each milestone before starting the next.

### M2 review — done

**Architect — strong:** correct reverse paint order (series 0 on top), bounds-checked braille
writes, padded-range edge continuity, tidy borrow scoping around `ctx.sub`. Gate clean.

**Architect — findings:**
- (fixed) `paint_empty` used non-saturating `w/2 - msg.len()/2`; now `saturating_sub`.
- (accepted) per-frame allocation of collected/stacked/scaled/grid — fine for charts; revisit
  only if a profile shows it.

**Product/API — strong:** `view.mount(dom) -> NodeId` + `view.with(|c| …)` is a clean consumer
surface; static + streaming both covered.

**Product/API — findings:**
- (fixed) `StackMode` was exported with no setter and no effect — removed from the public surface
  until the stacking transform lands (no marketing of unshipped work).
- (follow-up, non-blocking) `add_series` requires an explicit `Color` while `Series::line`
  auto-assigns from the palette — add an auto-color streaming helper for consistency.
- (follow-up, M6) `TimeSeriesView::with` mutates but does not request a repaint; documented at the
  API, to be ergonomized when interaction lands.

No blocking findings. Cleared to start M3.

### M3/M4/M5 reviews — done (no blockers)

- **M3 (sparkline):** clean reuse of the braille grid; pure `scale()` is the test seam. No findings.
- **M4 (bar/gauge):** shared `blocks::h_bar` keeps the two components DRY; pure `layout()` /
  `ratio()` / `fill_color()` are well-tested. Finding (accepted): `Gauge` is single-row only —
  multi-row/labeled-track layout is a future nicety, not needed for v1.
- **M5 (virtual table):** borrow scoping in `show_window` is sound; `drop_subtree` prevents arena
  leaks; virtualization is genuine (verified by `mounted_row_count`). Finding (recorded, not
  blocking): the rich virtual-table features are deliberately scoped out — README/STATE say "core"
  to avoid overselling. Repaint/scroll-wiring shares the M6 concern.
