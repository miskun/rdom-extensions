//! Charting components built on the rdom-tui `<canvas>` paint API.
//!
//! - [`data`] / [`axis`] — pure data model + axis math.
//! - [`braille`] / [`blocks`] — sub-cell line + block-fill rasterizers
//!   (crate-internal).
//! - [`time_series`] — braille line chart mounted on a `<canvas>`.
//! - [`sparkline`] — compact single-series line, no chrome.
//! - [`bar`] — horizontal labeled bar chart.
//! - [`gauge`] — linear gauge with colored zones.

pub mod axis;
pub mod bar;
pub mod data;
pub mod gauge;
pub mod sparkline;
pub mod time_series;

pub(crate) mod blocks;
pub(crate) mod braille;

pub use bar::{Bar, BarChart, BarChartView};
pub use data::{
    ConnectPolicy, DataPoint, Guideline, Series, SeriesStyle, TimeRange, XAxisConfig, YAxisConfig,
};
pub use gauge::{Gauge, GaugeView, GaugeZone};
pub use sparkline::{Sparkline, SparklineView};
pub use time_series::{TimeSeriesChart, TimeSeriesView};
