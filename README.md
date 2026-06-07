# rdom-charts

Terminal **charts** for [rdom](https://github.com/miskun/rdom), the browser-faithful DOM for
terminal applications: a time-series line chart, a sparkline, a bar chart, and a rich gauge.

Each chart paints onto a `<canvas>` element through rdom-tui's public API — sub-cell rasterized (a
2×4 **braille** dot grid for lines, eighth-**block** glyphs for bars) — so curves and fills look
smooth in a cell grid. Nothing reaches into rdom internals.

> Looking for a table? It lives in the separate [`rdom-virtualtable`](https://github.com/miskun/rdom-virtualtable) crate.

## Install

```toml
[dependencies]
rdom-charts = "0.1"
rdom-tui = "0.3.14"
```

## Try it

```bash
cargo run --example timeseries_gallery   # ← start here: 7 demos (1-7 to switch): smooth, spiky
                                         #   (100ms/150ms thresholds), single, dense, live,
                                         #   live-spotty, empty
cargo run --example interactive_chart     # zoom/pan/scrub a single chart
cargo run --example live_chart            # a chart streaming in real time
cargo run --example dashboard             # all chart components on one screen
```

Controls: `+`/`-` zoom · `h`/`l` or ←/→ pan · `0` reset · wheel zoom · drag pan · Ctrl-C quit.

## Components

| Component | What it does |
|---|---|
| `TimeSeriesChart` / `TimeSeriesView` | Braille line chart: static + streaming, EMA smoothing, follow/zoom/pan, `Guideline` threshold lines, nice-tick axes, legend |
| `Sparkline` / `SparklineView` | Compact single-series line, no chrome, auto/pinned range, NaN gaps |
| `BarChart` / `BarChartView` | Horizontal labeled bars, eighth-block fill, auto/pinned scale, value readout |
| `Gauge` / `GaugeView` | Linear gauge with colored value zones, label + readout |

## Example

```rust
use rdom_charts::{DataPoint, Series, TimeSeriesChart, TimeSeriesView};
use rdom_tui::{Size, TuiDom};

let series = vec![Series::line(
    "cpu",
    (0..120).map(|i| DataPoint::new(i as f64, (i as f64 * 0.1).sin() * 40.0 + 50.0)).collect(),
)];
let view = TimeSeriesView::new(TimeSeriesChart::new_static(series));

let mut dom = TuiDom::new();
let root = dom.root();
let canvas = view.mount(&mut dom);
dom.append_child(root, canvas).unwrap();
dom.node_mut(canvas).set_width(Size::Flex(1)).set_height(Size::Flex(1));

// Stream new samples later, then ask the runtime to repaint:
view.with(|chart| { chart.push_points(0, &[DataPoint::new(120.0, 72.0)]); chart.tick(120.0); });
```

## License

MIT.
