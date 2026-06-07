//! Chart data model: `DataPoint`, `TimeRange`, `Series`, `SeriesBuffer`.
//!
//! The buffer keeps points sorted by timestamp, deduplicates on equal
//! timestamps, bounds memory, and tracks loaded ranges for lazy backfill.

use rdom_tui::Color;

use crate::palette::series_color;

/// A single data point. `value == NaN` marks a gap (line breaks there).
#[derive(Clone, Debug, PartialEq)]
pub struct DataPoint {
    /// Epoch seconds.
    pub timestamp: f64,
    /// Sample value; `NaN` = gap.
    pub value: f64,
}

impl DataPoint {
    /// A point at `timestamp` (epoch seconds) with `value` (`NaN` = gap).
    pub fn new(timestamp: f64, value: f64) -> Self {
        Self { timestamp, value }
    }
}

/// A time range (visible window, loaded range, etc.).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeRange {
    /// Inclusive start (epoch seconds).
    pub start: f64,
    /// Inclusive end (epoch seconds).
    pub end: f64,
}

impl TimeRange {
    /// A range spanning `[start, end]` (epoch seconds).
    pub fn new(start: f64, end: f64) -> Self {
        Self { start, end }
    }
    /// Width of the range in seconds (`end - start`).
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
    /// Whether `t` falls within `[start, end]` (inclusive).
    pub fn contains(&self, t: f64) -> bool {
        t >= self.start && t <= self.end
    }
    /// Whether this range overlaps `other` (half-open comparison).
    pub fn overlaps(&self, other: &TimeRange) -> bool {
        self.start < other.end && other.start < self.end
    }
}

impl Default for TimeRange {
    fn default() -> Self {
        Self {
            start: 0.0,
            end: 0.0,
        }
    }
}

/// Rendering style per series.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SeriesStyle {
    /// A connected line through the data points.
    #[default]
    Line,
    // Future: Area, StepLine, StepArea
}

/// How to handle missing data points (NaN or absent).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ConnectPolicy {
    /// Break the line across gaps.
    #[default]
    Gap,
    // Future: Connect, Zero
}

// `StackMode` (None/Stacked/Percent) will land with the stacking
// transform; not exposed until it has an effect.

/// Series definition supplied by callers.
#[derive(Clone, Debug)]
pub struct Series {
    /// Display name (shown in the legend).
    pub name: String,
    /// Explicit color, or `None` to auto-assign from the palette.
    pub color: Option<Color>,
    /// The data points (any order; the buffer sorts + deduplicates them).
    pub data: Vec<DataPoint>,
    /// How the series is drawn.
    pub style: SeriesStyle,
    /// How gaps (`NaN` / missing points) are handled.
    pub connect: ConnectPolicy,
}

impl Series {
    /// Convenience: a line series with an auto-assigned palette color.
    pub fn line(name: impl Into<String>, data: Vec<DataPoint>) -> Self {
        Self {
            name: name.into(),
            color: None,
            data,
            style: SeriesStyle::Line,
            connect: ConnectPolicy::Gap,
        }
    }

    /// Set an explicit color (builder style).
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

/// Horizontal reference line drawn across the plot.
#[derive(Clone, Debug)]
pub struct Guideline {
    /// Y value the line is drawn at.
    pub y_value: f64,
    /// Line + label color.
    pub color: Color,
    /// Optional label drawn in the gutter at the line's row.
    pub label: Option<String>,
}

/// Y-axis configuration.
#[derive(Default)]
pub struct YAxisConfig {
    /// Pinned axis minimum, or `None` to auto-fit the data.
    pub min: Option<f64>,
    /// Pinned axis maximum, or `None` to auto-fit the data.
    pub max: Option<f64>,
    /// Tick-label formatter, or `None` for the default (`format_y_value`).
    pub format: Option<fn(f64) -> String>,
}

/// X-axis configuration.
#[derive(Default)]
pub struct XAxisConfig {
    /// Tick-label formatter, or `None` for the default (`format_timestamp`).
    pub format: Option<fn(f64) -> String>,
}

pub(crate) const DEFAULT_MAX_POINTS: usize = 10_000;

/// Internal per-series buffer. Sorted by timestamp, memory-bounded.
pub(crate) struct SeriesBuffer {
    pub name: String,
    pub color: Color,
    pub style: SeriesStyle,
    pub connect: ConnectPolicy,
    /// Points sorted by timestamp.
    pub points: Vec<DataPoint>,
    /// Loaded time ranges (sorted, non-overlapping).
    pub loaded_ranges: Vec<TimeRange>,
    /// Max points to retain before evicting the oldest.
    pub max_points: usize,
}

impl SeriesBuffer {
    pub fn new(name: String, color: Color, style: SeriesStyle, connect: ConnectPolicy) -> Self {
        Self {
            name,
            color,
            style,
            connect,
            points: Vec::new(),
            loaded_ranges: Vec::new(),
            max_points: DEFAULT_MAX_POINTS,
        }
    }

    pub fn from_series(s: Series, index: usize) -> Self {
        let color = s.color.unwrap_or_else(|| series_color(index));
        let mut buf = Self::new(s.name, color, s.style, s.connect);
        if !s.data.is_empty() {
            buf.push_points(&s.data);
        }
        buf
    }

    /// Push points, maintaining sorted order and deduplicating on equal
    /// timestamps. Single-point pushes take a fast insert path; bulk
    /// pushes sort-merge.
    pub fn push_points(&mut self, points: &[DataPoint]) {
        if points.is_empty() {
            return;
        }

        if points.len() == 1 {
            let p = &points[0];
            let idx = self
                .points
                .partition_point(|existing| existing.timestamp < p.timestamp);
            if idx < self.points.len() && (self.points[idx].timestamp - p.timestamp).abs() < 1e-9 {
                self.points[idx].value = p.value;
            } else {
                self.points.insert(idx, p.clone());
            }
        } else {
            let mut incoming: Vec<DataPoint> = points.to_vec();
            incoming.sort_by(|a, b| {
                a.timestamp
                    .partial_cmp(&b.timestamp)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });

            let mut merged = Vec::with_capacity(self.points.len() + incoming.len());
            let mut i = 0;
            let mut j = 0;
            while i < self.points.len() && j < incoming.len() {
                if (self.points[i].timestamp - incoming[j].timestamp).abs() < 1e-9 {
                    merged.push(incoming[j].clone()); // incoming wins
                    i += 1;
                    j += 1;
                } else if self.points[i].timestamp < incoming[j].timestamp {
                    merged.push(self.points[i].clone());
                    i += 1;
                } else {
                    merged.push(incoming[j].clone());
                    j += 1;
                }
            }
            merged.extend_from_slice(&self.points[i..]);
            merged.extend_from_slice(&incoming[j..]);
            self.points = merged;
        }

        if self.points.len() > self.max_points {
            let excess = self.points.len() - self.max_points;
            self.points.drain(..excess);
        }
    }

    /// Mark a range as loaded, merging into the existing coverage.
    pub fn mark_loaded(&mut self, range: &TimeRange) {
        let insert_at = self.loaded_ranges.partition_point(|r| r.end < range.start);
        let mut merged = *range;
        let mut remove_end = insert_at;
        while remove_end < self.loaded_ranges.len()
            && self.loaded_ranges[remove_end].start <= merged.end
        {
            merged.start = merged.start.min(self.loaded_ranges[remove_end].start);
            merged.end = merged.end.max(self.loaded_ranges[remove_end].end);
            remove_end += 1;
        }
        self.loaded_ranges
            .splice(insert_at..remove_end, std::iter::once(merged));
    }

    /// Is the entire range covered by `loaded_ranges`?
    pub fn is_loaded(&self, range: &TimeRange) -> bool {
        self.loaded_ranges
            .iter()
            .any(|lr| lr.start <= range.start && lr.end >= range.end)
    }

    /// Points strictly within a time range.
    #[cfg(test)]
    pub fn points_in_range(&self, range: &TimeRange) -> &[DataPoint] {
        let start = self.points.partition_point(|p| p.timestamp < range.start);
        let end = self.points.partition_point(|p| p.timestamp <= range.end);
        &self.points[start..end]
    }

    /// Points within a range, plus one neighbor each side for edge
    /// continuity (so lines reach the plot border).
    pub fn points_in_range_padded(&self, range: &TimeRange) -> &[DataPoint] {
        let start = self.points.partition_point(|p| p.timestamp < range.start);
        let end = self.points.partition_point(|p| p.timestamp <= range.end);
        let padded_start = start.saturating_sub(1);
        let padded_end = (end + 1).min(self.points.len());
        &self.points[padded_start..padded_end]
    }

    /// Time extent of the buffered points.
    pub fn time_extent(&self) -> Option<TimeRange> {
        if self.points.is_empty() {
            return None;
        }
        Some(TimeRange::new(
            self.points.first().unwrap().timestamp,
            self.points.last().unwrap().timestamp,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn buf() -> SeriesBuffer {
        SeriesBuffer::new(
            "test".into(),
            series_color(0),
            SeriesStyle::Line,
            ConnectPolicy::Gap,
        )
    }

    #[test]
    fn push_sorts() {
        let mut b = buf();
        b.push_points(&[
            DataPoint::new(3.0, 30.0),
            DataPoint::new(1.0, 10.0),
            DataPoint::new(2.0, 20.0),
        ]);
        assert_eq!(b.points.len(), 3);
        assert_eq!(b.points[0].timestamp, 1.0);
        assert_eq!(b.points[2].timestamp, 3.0);
    }

    #[test]
    fn push_deduplicates() {
        let mut b = buf();
        b.push_points(&[DataPoint::new(1.0, 10.0)]);
        b.push_points(&[DataPoint::new(1.0, 99.0)]);
        assert_eq!(b.points.len(), 1);
        assert_eq!(b.points[0].value, 99.0);
    }

    #[test]
    fn bulk_merge() {
        let mut b = buf();
        b.push_points(&[
            DataPoint::new(1.0, 1.0),
            DataPoint::new(3.0, 3.0),
            DataPoint::new(5.0, 5.0),
        ]);
        b.push_points(&[DataPoint::new(2.0, 2.0), DataPoint::new(4.0, 4.0)]);
        assert_eq!(b.points.len(), 5);
        for (i, p) in b.points.iter().enumerate() {
            assert_eq!(p.timestamp, (i + 1) as f64);
        }
    }

    #[test]
    fn evicts_oldest() {
        let mut b = buf();
        b.max_points = 3;
        b.push_points(&[
            DataPoint::new(1.0, 1.0),
            DataPoint::new(2.0, 2.0),
            DataPoint::new(3.0, 3.0),
            DataPoint::new(4.0, 4.0),
            DataPoint::new(5.0, 5.0),
        ]);
        assert_eq!(b.points.len(), 3);
        assert_eq!(b.points[0].timestamp, 3.0);
        assert_eq!(b.points[2].timestamp, 5.0);
    }

    #[test]
    fn points_in_range_inclusive() {
        let mut b = buf();
        b.push_points(&[
            DataPoint::new(1.0, 1.0),
            DataPoint::new(2.0, 2.0),
            DataPoint::new(3.0, 3.0),
            DataPoint::new(4.0, 4.0),
            DataPoint::new(5.0, 5.0),
        ]);
        let pts = b.points_in_range(&TimeRange::new(2.0, 4.0));
        assert_eq!(pts.len(), 3);
        assert_eq!(pts[0].timestamp, 2.0);
        assert_eq!(pts[2].timestamp, 4.0);
    }

    #[test]
    fn padded_range_adds_neighbors() {
        let mut b = buf();
        b.push_points(&[
            DataPoint::new(1.0, 1.0),
            DataPoint::new(2.0, 2.0),
            DataPoint::new(3.0, 3.0),
            DataPoint::new(4.0, 4.0),
            DataPoint::new(5.0, 5.0),
        ]);
        let pts = b.points_in_range_padded(&TimeRange::new(2.0, 4.0));
        // 2..=4 is 3 points; padding adds the neighbor at 1.0 and 5.0.
        assert_eq!(pts.first().unwrap().timestamp, 1.0);
        assert_eq!(pts.last().unwrap().timestamp, 5.0);
    }

    #[test]
    fn loaded_ranges_merge() {
        let mut b = buf();
        b.mark_loaded(&TimeRange::new(0.0, 10.0));
        assert!(b.is_loaded(&TimeRange::new(2.0, 8.0)));
        assert!(!b.is_loaded(&TimeRange::new(5.0, 15.0)));
        b.mark_loaded(&TimeRange::new(8.0, 20.0));
        assert!(b.is_loaded(&TimeRange::new(0.0, 20.0)));
        assert_eq!(b.loaded_ranges.len(), 1);
    }

    #[test]
    fn time_extent_spans_points() {
        let mut b = buf();
        assert_eq!(b.time_extent(), None);
        b.push_points(&[DataPoint::new(3.0, 1.0), DataPoint::new(9.0, 2.0)]);
        assert_eq!(b.time_extent(), Some(TimeRange::new(3.0, 9.0)));
    }

    #[test]
    fn from_series_assigns_palette_color() {
        let s = Series::line("CPU", vec![DataPoint::new(1.0, 50.0)]);
        let b = SeriesBuffer::from_series(s, 1);
        assert_eq!(b.color, series_color(1));
        assert_eq!(b.points.len(), 1);
    }

    #[test]
    fn from_series_respects_explicit_color() {
        let s = Series::line("CPU", vec![]).with_color(Color::Rgb(1, 2, 3));
        let b = SeriesBuffer::from_series(s, 0);
        assert_eq!(b.color, Color::Rgb(1, 2, 3));
    }
}
