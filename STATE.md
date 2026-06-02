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
- `rdom-tui` is a path dependency during co-development; switch to a crates.io version pin before
  publishing.

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

### M6 — Interaction + examples ⏳
- Wire keyboard/mouse listeners (`install_interaction`) so charts zoom/pan/follow from events,
  not just programmatic calls.
- Runnable examples driving the `App` event loop (depends on mapping the `App`/`Terminal` run API).

## Open questions / risks

- **Publish form:** path-dep vs crates.io pin. Resolve before first publish.
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
