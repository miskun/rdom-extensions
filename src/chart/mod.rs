//! Charting components built on the rdom-tui `<canvas>` paint API.
//!
//! - [`data`] / [`axis`] — pure data model + axis math.
//! - [`braille`] — sub-cell line rasterizer (crate-internal).
//! - [`time_series`] — braille line chart mounted on a `<canvas>`.
//! - [`sparkline`] — compact single-series line, no chrome.
//!
//! More components (bar, gauge) layer on top in later milestones; see
//! `STATE.md`.

pub mod axis;
pub mod data;
pub mod sparkline;
pub mod time_series;

pub(crate) mod braille;

pub use data::{
    ConnectPolicy, DataPoint, Guideline, Series, SeriesStyle, TimeRange, XAxisConfig, YAxisConfig,
};
pub use sparkline::{Sparkline, SparklineView};
pub use time_series::{TimeSeriesChart, TimeSeriesView};
