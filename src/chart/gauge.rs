//! Linear gauge with colored zones.
//!
//! Richer than the native `<progress>`/`<meter>`: a horizontal track
//! filled to `value` within `[min, max]`, where the fill color is
//! chosen by which **zone** the value falls in (e.g. green up to 70,
//! amber to 90, red beyond), plus an optional label and a numeric
//! readout.
//!
//! ```text
//! cpu  ███████████████▌            72%
//! ```

use std::cell::RefCell;
use std::rc::Rc;

use rdom_tui::runtime::builtins::canvas::{self, RenderContext};
use rdom_tui::{Color, NodeId, Style, TuiDom};

use super::blocks::h_bar;
use crate::palette::{LABEL, MUTED, series_color};

/// A colored band covering values up to `upto` (inclusive). Zones are
/// matched in ascending `upto` order; the first whose `upto >= value`
/// wins.
#[derive(Clone, Copy, Debug)]
pub struct GaugeZone {
    pub upto: f64,
    pub color: Color,
}

impl GaugeZone {
    pub fn new(upto: f64, color: Color) -> Self {
        Self { upto, color }
    }
}

/// A linear gauge.
pub struct Gauge {
    value: f64,
    min: f64,
    max: f64,
    zones: Vec<GaugeZone>,
    label: Option<String>,
    show_value: bool,
    value_fmt: fn(f64) -> String,
}

fn default_pct(v: f64) -> String {
    format!("{v:.0}")
}

impl Gauge {
    /// A gauge for `value` within `[min, max]`.
    pub fn new(value: f64, min: f64, max: f64) -> Self {
        Self {
            value,
            min,
            max,
            zones: Vec::new(),
            label: None,
            show_value: true,
            value_fmt: default_pct,
        }
    }

    /// Set colored zones (matched in ascending `upto` order).
    pub fn with_zones(mut self, mut zones: Vec<GaugeZone>) -> Self {
        zones.sort_by(|a, b| {
            a.upto
                .partial_cmp(&b.upto)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        self.zones = zones;
        self
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn without_value(mut self) -> Self {
        self.show_value = false;
        self
    }

    pub fn with_value_format(mut self, fmt: fn(f64) -> String) -> Self {
        self.value_fmt = fmt;
        self
    }

    pub fn set_value(&mut self, value: f64) {
        self.value = value;
    }

    /// Fill fraction in `[0, 1]`.
    fn ratio(&self) -> f64 {
        let span = self.max - self.min;
        if span <= 0.0 {
            return 0.0;
        }
        ((self.value - self.min) / span).clamp(0.0, 1.0)
    }

    /// Fill color: the first zone whose `upto >= value`, else the last
    /// zone, else the default series color.
    fn fill_color(&self) -> Color {
        if let Some(z) = self.zones.iter().find(|z| self.value <= z.upto) {
            return z.color;
        }
        self.zones
            .last()
            .map(|z| z.color)
            .unwrap_or_else(|| series_color(0))
    }

    /// Paint the gauge into a canvas `RenderContext` (single row).
    pub fn paint(&self, ctx: &mut RenderContext<'_>) {
        let w = ctx.width();
        let h = ctx.height();
        if w < 4 || h == 0 {
            return;
        }
        let y = 0;
        let mut x = 0u16;

        // Label gutter (at most a third of the width).
        if let Some(label) = &self.label {
            let lw = (label.chars().count() as u16 + 1).min(w / 3);
            ctx.text(x, y, label, Style::new().fg(LABEL));
            x += lw;
        }

        // Value readout reserved on the right.
        let value_txt = if self.show_value {
            Some((self.value_fmt)(self.value))
        } else {
            None
        };
        let value_w = value_txt
            .as_ref()
            .map(|t| t.chars().count() as u16 + 1)
            .unwrap_or(0);

        let track_w = w.saturating_sub(x + value_w);
        if track_w == 0 {
            return;
        }

        // Track background, then the colored fill over it.
        ctx.text(x, y, &" ".repeat(track_w as usize), Style::new().fg(MUTED));
        let fill = h_bar(track_w, self.ratio());
        ctx.text(x, y, &fill, Style::new().fg(self.fill_color()));

        if let Some(txt) = value_txt {
            ctx.text(x + track_w + 1, y, &txt, Style::new().fg(LABEL));
        }
    }
}

/// A shareable handle that owns a [`Gauge`] and renders it onto a
/// `<canvas>` element.
#[derive(Clone)]
pub struct GaugeView {
    inner: Rc<RefCell<Gauge>>,
}

impl GaugeView {
    pub fn new(gauge: Gauge) -> Self {
        Self {
            inner: Rc::new(RefCell::new(gauge)),
        }
    }

    pub fn mount(&self, dom: &mut TuiDom) -> NodeId {
        let id = dom.create_element("canvas");
        let inner = self.inner.clone();
        canvas::set_paint(dom, id, move |_dom, ctx| {
            inner.borrow().paint(ctx);
        });
        id
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut Gauge) -> R) -> R {
        f(&mut self.inner.borrow_mut())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ratio_maps_value_into_unit_interval() {
        assert_eq!(Gauge::new(0.0, 0.0, 100.0).ratio(), 0.0);
        assert_eq!(Gauge::new(50.0, 0.0, 100.0).ratio(), 0.5);
        assert_eq!(Gauge::new(100.0, 0.0, 100.0).ratio(), 1.0);
    }

    #[test]
    fn ratio_clamps_out_of_range() {
        assert_eq!(Gauge::new(-10.0, 0.0, 100.0).ratio(), 0.0);
        assert_eq!(Gauge::new(150.0, 0.0, 100.0).ratio(), 1.0);
    }

    #[test]
    fn ratio_handles_degenerate_span() {
        assert_eq!(Gauge::new(5.0, 10.0, 10.0).ratio(), 0.0);
    }

    #[test]
    fn fill_color_picks_matching_zone() {
        let green = Color::Rgb(0, 255, 0);
        let amber = Color::Rgb(255, 200, 0);
        let red = Color::Rgb(255, 0, 0);
        let zones = vec![
            GaugeZone::new(70.0, green),
            GaugeZone::new(90.0, amber),
            GaugeZone::new(100.0, red),
        ];
        assert_eq!(
            Gauge::new(50.0, 0.0, 100.0)
                .with_zones(zones.clone())
                .fill_color(),
            green
        );
        assert_eq!(
            Gauge::new(80.0, 0.0, 100.0)
                .with_zones(zones.clone())
                .fill_color(),
            amber
        );
        assert_eq!(
            Gauge::new(95.0, 0.0, 100.0).with_zones(zones).fill_color(),
            red
        );
    }

    #[test]
    fn fill_color_beyond_last_zone_uses_last() {
        let red = Color::Rgb(255, 0, 0);
        let zones = vec![GaugeZone::new(90.0, red)];
        assert_eq!(
            Gauge::new(99.0, 0.0, 100.0).with_zones(zones).fill_color(),
            red
        );
    }

    #[test]
    fn with_zones_sorts_unordered_input() {
        let a = Color::Rgb(1, 0, 0);
        let b = Color::Rgb(2, 0, 0);
        // Provided out of order; lowest threshold should still match first.
        let g = Gauge::new(10.0, 0.0, 100.0)
            .with_zones(vec![GaugeZone::new(90.0, b), GaugeZone::new(20.0, a)]);
        assert_eq!(g.fill_color(), a);
    }

    #[test]
    fn fill_color_no_zones_is_default() {
        assert_eq!(Gauge::new(50.0, 0.0, 100.0).fill_color(), series_color(0));
    }
}
