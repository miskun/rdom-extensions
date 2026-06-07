//! Horizontal bar chart: one labeled bar per row.
//!
//! Each row is `label │ ████████▌      value`. Bars use the eighth-block
//! fill ([`blocks::h_bar`]) for sub-cell precision. Rows beyond the
//! canvas height are clipped (and reported via the row count, so callers
//! can size the canvas to the data).

use std::cell::RefCell;
use std::rc::Rc;

use rdom_tui::runtime::builtins::canvas::{self, RenderContext};
use rdom_tui::{Color, NodeId, Style, TuiDom};

use super::axis::format_y_value;
use super::blocks::h_bar;
use crate::palette::{LABEL, series_color};

/// One labeled bar.
#[derive(Clone, Debug)]
pub struct Bar {
    /// Row label drawn in the left gutter.
    pub label: String,
    /// Bar value (drives the fill ratio against the chart's max).
    pub value: f64,
    /// Explicit color, or `None` to auto-assign from the palette.
    pub color: Option<Color>,
}

impl Bar {
    /// A bar labeled `label` with the given `value` and a palette color.
    pub fn new(label: impl Into<String>, value: f64) -> Self {
        Self {
            label: label.into(),
            value,
            color: None,
        }
    }

    /// Set an explicit bar color (builder style).
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

/// A horizontal bar chart.
pub struct BarChart {
    bars: Vec<Bar>,
    max: Option<f64>,
    show_values: bool,
    value_fmt: fn(f64) -> String,
}

impl BarChart {
    /// A bar chart over `bars` (auto-scaled, value readouts shown).
    pub fn new(bars: Vec<Bar>) -> Self {
        Self {
            bars,
            max: None,
            show_values: true,
            value_fmt: format_y_value,
        }
    }

    /// Pin the full-scale value (default: the largest bar).
    pub fn with_max(mut self, max: f64) -> Self {
        self.max = Some(max);
        self
    }

    /// Hide the trailing numeric readout.
    pub fn without_values(mut self) -> Self {
        self.show_values = false;
        self
    }

    /// Override the value formatter for the trailing readout.
    pub fn with_value_format(mut self, fmt: fn(f64) -> String) -> Self {
        self.value_fmt = fmt;
        self
    }

    /// Replace the bars (for live updates).
    pub fn set_bars(&mut self, bars: Vec<Bar>) {
        self.bars = bars;
    }

    /// Full-scale value: the override, or the largest bar value, floored
    /// to a positive number so ratios stay finite.
    fn effective_max(&self) -> f64 {
        let m = self.max.unwrap_or_else(|| {
            self.bars
                .iter()
                .map(|b| b.value)
                .filter(|v| v.is_finite())
                .fold(f64::NEG_INFINITY, f64::max)
        });
        if m.is_finite() && m > 0.0 { m } else { 1.0 }
    }

    /// Column layout for a given canvas width: `(label_w, bar_w, value_w)`.
    /// Pure — the unit of testing for the partitioning.
    fn layout(&self, w: u16) -> (u16, u16, u16) {
        let longest_label = self
            .bars
            .iter()
            .map(|b| b.label.chars().count())
            .max()
            .unwrap_or(0) as u16;
        // Label gutter: clamp so it never eats more than a third.
        let label_w = longest_label.min(w / 3);

        let value_w = if self.show_values {
            let fmt = self.value_fmt;
            let widest = self
                .bars
                .iter()
                .map(|b| fmt(b.value).chars().count())
                .max()
                .unwrap_or(0) as u16;
            (widest + 1).min(w / 4)
        } else {
            0
        };

        // 1 column separates label from bar.
        let used = label_w + 1 + value_w;
        let bar_w = w.saturating_sub(used);
        (label_w, bar_w, value_w)
    }

    /// Number of bars that fit in `height` rows (the rest are clipped).
    pub fn visible_rows(&self, height: u16) -> usize {
        self.bars.len().min(height as usize)
    }

    /// Paint the chart into a canvas `RenderContext`.
    pub fn paint(&self, ctx: &mut RenderContext<'_>) {
        let w = ctx.width();
        let h = ctx.height();
        if w < 4 || h == 0 || self.bars.is_empty() {
            return;
        }
        let (label_w, bar_w, _value_w) = self.layout(w);
        if bar_w == 0 {
            return;
        }
        let max = self.effective_max();
        let label_style = Style::new().fg(LABEL);

        for (i, bar) in self.bars.iter().take(h as usize).enumerate() {
            let y = i as u16;
            // Label (left-aligned, clipped to the gutter by `text`).
            if label_w > 0 {
                ctx.text(0, y, &bar.label, label_style);
            }
            // Bar fill.
            let ratio = if bar.value.is_finite() {
                bar.value / max
            } else {
                0.0
            };
            let color = bar.color.unwrap_or_else(|| series_color(i));
            let fill = h_bar(bar_w, ratio);
            ctx.text(label_w + 1, y, &fill, Style::new().fg(color));
            // Trailing value readout.
            if self.show_values {
                let txt = (self.value_fmt)(bar.value);
                let vx = label_w + 1 + bar_w + 1;
                ctx.text(vx, y, &txt, label_style);
            }
        }
    }
}

/// A shareable handle that owns a [`BarChart`] and renders it onto a
/// `<canvas>` element.
#[derive(Clone)]
pub struct BarChartView {
    inner: Rc<RefCell<BarChart>>,
}

impl BarChartView {
    /// Wrap a [`BarChart`] in a shareable view handle.
    pub fn new(chart: BarChart) -> Self {
        Self {
            inner: Rc::new(RefCell::new(chart)),
        }
    }

    /// Create a `<canvas>` wired to paint this chart; returns its `NodeId`
    /// for the caller to append and size.
    pub fn mount(&self, dom: &mut TuiDom) -> NodeId {
        let id = dom.create_element("canvas");
        let inner = self.inner.clone();
        canvas::set_paint(dom, id, move |_dom, ctx| {
            inner.borrow().paint(ctx);
        });
        id
    }

    /// Borrow the chart mutably to update it (e.g. `set_bars`).
    pub fn with<R>(&self, f: impl FnOnce(&mut BarChart) -> R) -> R {
        f(&mut self.inner.borrow_mut())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chart() -> BarChart {
        BarChart::new(vec![
            Bar::new("alpha", 10.0),
            Bar::new("b", 20.0),
            Bar::new("gamma", 5.0),
        ])
    }

    #[test]
    fn effective_max_is_largest_bar() {
        assert_eq!(chart().effective_max(), 20.0);
    }

    #[test]
    fn effective_max_override_wins() {
        assert_eq!(chart().with_max(100.0).effective_max(), 100.0);
    }

    #[test]
    fn effective_max_floors_to_positive() {
        let c = BarChart::new(vec![Bar::new("z", 0.0)]);
        assert_eq!(c.effective_max(), 1.0);
        let neg = BarChart::new(vec![Bar::new("z", -5.0)]);
        assert_eq!(neg.effective_max(), 1.0);
    }

    #[test]
    fn layout_partitions_width() {
        let (label_w, bar_w, value_w) = chart().layout(40);
        assert!((1..=40 / 3).contains(&label_w));
        assert!(value_w >= 1);
        assert_eq!(label_w + 1 + bar_w + value_w, 40);
    }

    #[test]
    fn layout_label_capped_at_third() {
        let c = BarChart::new(vec![Bar::new("a_very_long_label_indeed", 1.0)]);
        let (label_w, _, _) = c.layout(30);
        assert_eq!(label_w, 30 / 3);
    }

    #[test]
    fn without_values_drops_value_column() {
        let (_, _, value_w) = chart().without_values().layout(40);
        assert_eq!(value_w, 0);
    }

    #[test]
    fn visible_rows_clips_to_height() {
        assert_eq!(chart().visible_rows(2), 2);
        assert_eq!(chart().visible_rows(10), 3);
    }
}
