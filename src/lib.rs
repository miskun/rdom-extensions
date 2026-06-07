//! # rdom-charts
//!
//! Terminal charts for [rdom](https://github.com/miskun/rdom), the
//! browser-faithful DOM for terminal applications: a time-series line
//! chart, a sparkline, a bar chart, and a rich gauge.
//!
//! Each chart paints onto a `<canvas>` element through rdom-tui's public
//! API — sub-cell rasterized (a 2×4 **braille** dot grid for lines, eighth
//! **block** glyphs for bars) — and never reaches into rdom internals, so
//! the crate evolves independently of the substrate. A `*View` handle
//! mounts a chart on a canvas and lets the app update it between frames:
//!
//! ```no_run
//! use rdom_charts::{Series, DataPoint, TimeSeriesChart, TimeSeriesView};
//! use rdom_tui::TuiDom;
//!
//! let chart = TimeSeriesChart::new_static(vec![Series::line(
//!     "cpu",
//!     (0..120).map(|i| DataPoint::new(i as f64, (i as f64 * 0.1).sin() * 40.0 + 50.0)).collect(),
//! )]);
//! let view = TimeSeriesView::new(chart);
//! let mut dom = TuiDom::new();
//! let canvas = view.mount(&mut dom); // a <canvas> NodeId — append + size it
//! # let _ = canvas;
//! ```
//!
//! Components: [`TimeSeriesChart`] / [`TimeSeriesView`] (with [`Guideline`]
//! threshold lines, EMA smoothing, follow/zoom/pan), [`Sparkline`],
//! [`BarChart`], [`Gauge`] (+ [`GaugeZone`]). See the `examples/` for
//! runnable demos.

#![deny(missing_docs)]

mod chart;
pub mod palette;

pub use chart::{
    Bar, BarChart, BarChartView, ConnectPolicy, DataPoint, Gauge, GaugeView, GaugeZone, Guideline,
    Series, SeriesStyle, Sparkline, SparklineView, TimeRange, TimeSeriesChart, TimeSeriesView,
    XAxisConfig, YAxisConfig,
};
