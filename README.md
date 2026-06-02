# rdom-extensions

Optional **data-visualization components** for [rdom](https://github.com/miskun/rdom) — the
browser-faithful DOM for terminal applications.

This crate is a **downstream consumer** of the rdom substrate, not part of it. The core rdom
workspace deliberately ships only native HTML element behaviors and *zero opinionated components*;
charts, sparklines, gauges, and virtualized tables are exactly the kind of higher-level components
that belong outside that publish set. They live here, built strictly on `rdom-tui`'s public API:

- the **`<canvas>` paint API** (`canvas::set_paint` + `RenderContext`) for sub-cell drawing —
  charts rasterize onto a 2×4 **braille** dot grid for 2× horizontal / 4× vertical resolution;
- **element builders + the cascade** for layout, sizing, and color;
- the **runtime event listeners** for interaction (zoom, pan, hover).

Nothing here reaches into rdom internals, so the crate evolves independently of the substrate.

## Status

| Component | State |
|---|---|
| Charting foundation — data model, axis math, braille rasterizer | ✅ shipped |
| **Time-series line chart** (`TimeSeriesChart` / `TimeSeriesView`) — static + streaming, windowing, follow, zoom/pan, smoothing, guidelines, legend, axes | ✅ shipped |
| **Sparkline** (`Sparkline` / `SparklineView`) — compact single-series line, no chrome, auto/pinned range, NaN gaps | ✅ shipped |
| **Bar chart** (`BarChart` / `BarChartView`) — horizontal labeled bars, eighth-block fill, auto/pinned scale, value readout | ✅ shipped |
| **Rich gauge** (`Gauge` / `GaugeView`) — linear gauge with colored value zones, label + readout | ✅ shipped |
| **Virtual table — core** (`VirtualTable` / `VirtualTableView`) — native `<table>`, materializes only the visible row window | ✅ shipped |
| Virtual table — rich features (sort, selection, column resize/reorder, scrollbar spacer) | ⏳ planned |
| **Runnable examples** (`dashboard`, `live_chart`, `interactive_chart`) | ✅ shipped |
| **Interaction wiring** — `TimeSeriesView::install_interaction` (keyboard zoom/pan, wheel-zoom, drag-pan) | ✅ shipped |

See `STATE.md` for the milestone plan and `specs/DESIGN.md` (in the rdom repo) for the
substrate-first rationale.

## Try it

```bash
cargo run --example dashboard          # every component on one screen (Ctrl-C to quit)
cargo run --example live_chart         # a time-series streaming in real time
cargo run --example interactive_chart  # zoom/pan/scrub with keyboard + mouse
```

## Example

```rust
use rdom_extensions::chart::{DataPoint, Series, TimeSeriesChart, TimeSeriesView};
use rdom_tui::{Size, TuiDom};

// Build a chart from data.
let series = vec![Series::line(
    "cpu",
    (0..120)
        .map(|i| DataPoint::new(i as f64, (i as f64 * 0.1).sin() * 40.0 + 50.0))
        .collect(),
)];
let view = TimeSeriesView::new(TimeSeriesChart::new_static(series));

// Mount it as a <canvas> in any rdom tree.
let mut dom = TuiDom::new();
let root = dom.root();
let canvas = view.mount(&mut dom);
dom.append_child(root, canvas).unwrap();
dom.node_mut(canvas).set_width(Size::Flex(1)).set_height(Size::Flex(1));

// Stream new samples later, then ask the runtime to repaint:
view.with(|chart| {
    chart.push_points(0, &[DataPoint::new(120.0, 72.0)]);
    chart.tick(120.0);
});
```

## License

MIT.
