//! Sparkline: a compact single-series braille line with no chrome.
//!
//! A sparkline is the minimal chart — just the trend, sized to fit
//! inline (a table cell, a status bar, a label). It reuses the same
//! [`BrailleGrid`] rasterizer as the time-series chart but draws no
//! axes, gutter, or legend: the whole canvas is plot area.
//!
//! Values are evenly spaced across the width; `NaN` marks a gap (the
//! line breaks there). The vertical range auto-fits the finite values
//! unless pinned with [`with_range`](Sparkline::with_range).

use std::cell::RefCell;
use std::rc::Rc;

use rdom_tui::runtime::builtins::canvas::{self, RenderContext};
use rdom_tui::{Color, NodeId, TuiDom};

use super::braille::BrailleGrid;
use crate::palette::series_color;

/// A compact single-series line chart.
pub struct Sparkline {
    values: Vec<f64>,
    color: Color,
    min: Option<f64>,
    max: Option<f64>,
}

impl Sparkline {
    /// A sparkline over `values` (`NaN` = gap), with the default color.
    pub fn new(values: Vec<f64>) -> Self {
        Self {
            values,
            color: series_color(0),
            min: None,
            max: None,
        }
    }

    /// Set the line color (builder).
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Pin the vertical range instead of auto-fitting (builder).
    pub fn with_range(mut self, min: f64, max: f64) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    /// Replace the values (for streaming/live updates).
    pub fn set_values(&mut self, values: Vec<f64>) {
        self.values = values;
    }

    /// Effective `(min, max)` after overrides and degenerate handling.
    fn effective_range(&self) -> (f64, f64) {
        let mut lo = self.min.unwrap_or(f64::INFINITY);
        let mut hi = self.max.unwrap_or(f64::NEG_INFINITY);
        if self.min.is_none() || self.max.is_none() {
            for &v in &self.values {
                if v.is_finite() {
                    if self.min.is_none() {
                        lo = lo.min(v);
                    }
                    if self.max.is_none() {
                        hi = hi.max(v);
                    }
                }
            }
        }
        if !lo.is_finite() || !hi.is_finite() {
            return (0.0, 1.0);
        }
        if (hi - lo).abs() < 1e-12 {
            let pad = if lo.abs() < 1e-12 {
                1.0
            } else {
                lo.abs() * 0.1
            };
            return (lo - pad, hi + pad);
        }
        (lo, hi)
    }

    /// Map each value to a braille-space coordinate (`None` for gaps).
    /// Pure — the unit of testing for the scaling math.
    fn scale(&self, bw: i32, bh: i32) -> Vec<Option<(i32, i32)>> {
        let n = self.values.len();
        let (lo, hi) = self.effective_range();
        let range = hi - lo;
        self.values
            .iter()
            .enumerate()
            .map(|(i, &v)| {
                if !v.is_finite() {
                    return None;
                }
                let x_frac = if n > 1 {
                    i as f64 / (n - 1) as f64
                } else {
                    0.5
                };
                let v_frac = if range > 0.0 { (v - lo) / range } else { 0.5 };
                Some((
                    (x_frac * (bw - 1) as f64).round() as i32,
                    ((1.0 - v_frac) * (bh - 1) as f64).round() as i32,
                ))
            })
            .collect()
    }

    /// Paint the sparkline into a canvas `RenderContext`.
    pub fn paint(&self, ctx: &mut RenderContext<'_>) {
        let w = ctx.width();
        let h = ctx.height();
        if w == 0 || h == 0 || self.values.is_empty() {
            return;
        }
        let mut grid = BrailleGrid::new(w, h);
        let pts = self.scale(grid.braille_width(), grid.braille_height());
        let mut prev: Option<(i32, i32)> = None;
        for p in pts {
            match p {
                Some((bx, by)) => {
                    grid.set_dot(bx, by, self.color);
                    if let Some((px, py)) = prev {
                        grid.draw_line(px, py, bx, by, self.color);
                    }
                    prev = Some((bx, by));
                }
                None => prev = None,
            }
        }
        grid.paint(ctx);
    }
}

/// A shareable handle that owns a [`Sparkline`] and renders it onto a
/// `<canvas>` element. See [`TimeSeriesView`](super::TimeSeriesView) for
/// the same mount/update pattern.
#[derive(Clone)]
pub struct SparklineView {
    inner: Rc<RefCell<Sparkline>>,
}

impl SparklineView {
    /// Wrap a [`Sparkline`] in a shareable view handle.
    pub fn new(sparkline: Sparkline) -> Self {
        Self {
            inner: Rc::new(RefCell::new(sparkline)),
        }
    }

    /// Create a `<canvas>` wired to paint this sparkline; returns its id.
    pub fn mount(&self, dom: &mut TuiDom) -> NodeId {
        let id = dom.create_element("canvas");
        let inner = self.inner.clone();
        canvas::set_paint(dom, id, move |_dom, ctx| {
            inner.borrow().paint(ctx);
        });
        id
    }

    /// Borrow the sparkline mutably to update it.
    pub fn with<R>(&self, f: impl FnOnce(&mut Sparkline) -> R) -> R {
        f(&mut self.inner.borrow_mut())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_maps_within_grid_bounds() {
        let s = Sparkline::new((0..20).map(|i| (i as f64 * 0.5).sin()).collect());
        let (bw, bh) = (80, 40);
        for p in s.scale(bw, bh).into_iter().flatten() {
            assert!(p.0 >= 0 && p.0 < bw, "x {} out of [0,{bw})", p.0);
            assert!(p.1 >= 0 && p.1 < bh, "y {} out of [0,{bh})", p.1);
        }
    }

    #[test]
    fn scale_endpoints_span_width() {
        let s = Sparkline::new(vec![1.0, 2.0, 3.0, 4.0]);
        let pts = s.scale(80, 40);
        assert_eq!(pts.first().unwrap().unwrap().0, 0);
        assert_eq!(pts.last().unwrap().unwrap().0, 79);
    }

    #[test]
    fn min_value_sits_at_bottom_max_at_top() {
        // ascending values → first (min) at the bottom row, last (max) at top.
        let s = Sparkline::new(vec![0.0, 10.0]);
        let pts = s.scale(80, 40);
        let first = pts[0].unwrap();
        let last = pts[1].unwrap();
        assert_eq!(first.1, 39, "min should map to the bottom braille row");
        assert_eq!(last.1, 0, "max should map to the top braille row");
    }

    #[test]
    fn nan_is_a_gap() {
        let s = Sparkline::new(vec![1.0, f64::NAN, 3.0]);
        let pts = s.scale(80, 40);
        assert!(pts[0].is_some());
        assert!(pts[1].is_none());
        assert!(pts[2].is_some());
    }

    #[test]
    fn single_value_centers_horizontally() {
        let s = Sparkline::new(vec![5.0]);
        let pts = s.scale(80, 40);
        // x_frac 0.5 → mid of [0,79]
        assert_eq!(pts[0].unwrap().0, 40);
    }

    #[test]
    fn range_override_clamps_mapping() {
        let s = Sparkline::new(vec![50.0]).with_range(0.0, 100.0);
        let pts = s.scale(80, 40);
        // 50 of [0,100] → mid → ~row 20 (rounded from 0.5 * 39).
        let y = pts[0].unwrap().1;
        assert!(
            (18..=21).contains(&y),
            "midpoint should be mid-height, got {y}"
        );
    }
}
