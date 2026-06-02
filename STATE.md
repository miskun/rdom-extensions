# rdom-extensions — Project State

Living journal for the optional data-visualization crate built on top of rdom.

## Thesis

The rdom workspace ships a substrate (DOM + cascade + layout + paint + runtime) and native HTML
elements only — **zero opinionated components**, by an explicit non-negotiable rule in its
`CLAUDE.md` / `specs/DESIGN.md`. Data-viz components (charts, sparklines, gauges, virtual tables)
are opinionated components, so they live here as a **downstream consumer crate**, not in the rdom
publish set. We build strictly on `rdom-tui`'s public API (canvas paint, cascade, layout, event
listeners) and never touch rdom internals.

Origin: the components are ported from `../lens-k8s-tui` (`crates/lens-ui`), which built them on an
older, **ratatui-based** fork of rdom. The published rdom workspace has its own paint stack (no
ratatui). So this is a **port, not a move**: the algorithms (braille grid + Bresenham, data
buffers, nice-ticks, EMA) transfer; the rendering layer is rewritten against the canvas
`RenderContext`, and `ColorToken`/`Theme` becomes `rdom_tui::Color`/`Style`.

## Architecture

- One component = one `<canvas>` element + a paint closure that draws via `RenderContext`.
- Charts rasterize onto a `BrailleGrid` (2×4 dots/cell), then flush into a `ctx.sub(...)` rect.
- State lives behind `Rc<RefCell<…>>` (the `*View` types) so the paint closure can borrow it and
  the app can mutate it between frames, then request a repaint.
- `rdom-tui` is a plain crates.io dependency (**`"0.3.1"`** as of 2026-06-02) — no path dep, so
  this crate builds standalone and never reaches into the rdom source tree.

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
- `palette` — `Color` palette + `series_color()` (replaces lens `ColorToken`/`Theme`).
- `chart::data` — `DataPoint`, `TimeRange`, `Series`, `SeriesBuffer` (sorted/dedup/bounded/lazy).
- `chart::axis` — `nice_ticks`, `format_y_value`, `format_timestamp`.
- `chart::braille` — `BrailleGrid` + Bresenham + EMA + scale pipeline, retargeted to `RenderContext`.
- 24 unit tests ported and green.
- *Note:* M1 was folded into the M2 commit — a foundation with no consumer trips the
  `clippy -D warnings` dead-code gate, so it landed together with its first consumer.

### M2 — Time-series line chart ✅ (done)
- `chart::time_series` — `TimeSeriesChart` (pure state + `paint(&RenderContext)`) and
  `TimeSeriesView` (`Rc<RefCell>` handle; `mount(dom) -> NodeId`, `with(|chart| …)`).
- Full render pipeline ported: collect → stack → smooth → scale → render → decorate → paint,
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

**Scoped out of M5 (deliberate, not done):** this is the virtualization *core*, not the full lens
`VirtualTableComponent`. Deferred: automatic scroll → window recompute + a spacer so the scrollbar
reflects total rows; sorting; row/cell selection; column resize/reorder/hide; side-loaded data
sources; persistence callbacks. Tracked as follow-ups (candidate M7).

### M6 — Runnable examples ✅ (done); interaction ⏳
- `examples/dashboard.rs` — a static dashboard showing every component at once (two gauges, bar
  chart, sparkline, time-series with legend/axes, virtual table). `cargo run --example dashboard`.
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

## Open questions / risks

- **Publish form:** resolved — depends on crates.io `rdom-tui = "0.2"`, no path dep.
- **Repaint signaling:** `TimeSeriesView::with` mutates state but does not request a repaint;
  the app must call the runtime's redraw path. M6 should document/ergonomize this.
- **No runnable example yet** — behavior is currently proven by the headless integration test
  only. M6 adds an interactive example.

## Review gates

Per the rdom working agreement, run the Grumpy Chief Architect + Grumpy Chief Product/API passes at
the end of each milestone before starting the next.

### M2 review — done

**Architect — strong:** correct reverse paint order (series 0 on top), bounds-checked braille
writes, padded-range edge continuity, tidy borrow scoping around `ctx.sub`. Gate clean.

**Architect — findings:**
- (fixed) `paint_empty` used non-saturating `w/2 - msg.len()/2`; now `saturating_sub`.
- (accepted) per-frame allocation of collected/stacked/scaled/grid — same as upstream, fine for
  charts; revisit only if a profile shows it.

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
  blocking): the rich lens table features are deliberately scoped out — README/STATE say "core"
  to avoid overselling. Repaint/scroll-wiring shares the M6 concern.
