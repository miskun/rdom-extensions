//! Time-series line chart rendered on a `<canvas>` via a braille grid.
//!
//! Ported from the upstream lens `TimeSeriesComponent`. The render
//! pipeline is unchanged — collect → stack → smooth → scale → render →
//! decorate → paint — but the paint target is the rdom-tui canvas
//! [`RenderContext`] (local coords, `(0,0)` top-left) instead of a
//! ratatui `Buffer`, and colors are `rdom_tui::Color` instead of theme
//! tokens.
//!
//! Two tiers:
//! - **Static**: [`TimeSeriesChart::new_static`] — all data upfront.
//! - **Streaming**: [`TimeSeriesChart::new`] + [`add_series`] +
//!   [`push_points`] + [`tick`].
//!
//! [`add_series`]: TimeSeriesChart::add_series
//! [`push_points`]: TimeSeriesChart::push_points
//! [`tick`]: TimeSeriesChart::tick

use std::cell::RefCell;
use std::rc::Rc;

use rdom_tui::runtime::builtins::canvas::{self, RenderContext};
use rdom_tui::{Color, ListenerOptions, NodeId, Style, TuiDom, TuiNodeExt};

use super::axis::{format_timestamp, format_y_value, nice_ticks};
use super::braille::{
    BrailleGrid, ScaledPoint, ScaledSeries, StackedPoint, ema_smooth, render_series,
};
use super::data::{
    ConnectPolicy, DataPoint, Guideline, Series, SeriesBuffer, SeriesStyle, TimeRange, XAxisConfig,
    YAxisConfig,
};
use crate::palette::{LABEL, MUTED};

/// A time-series chart: data buffers + a visible window + render config.
///
/// This type owns no rdom state and never touches a terminal directly —
/// it is pure logic plus a [`paint`](Self::paint) method that draws into
/// a canvas [`RenderContext`]. Wrap it in a [`TimeSeriesView`] to mount
/// it on a `<canvas>` element.
pub struct TimeSeriesChart {
    series: Vec<SeriesBuffer>,
    guidelines: Vec<Guideline>,

    window: TimeRange,
    window_duration: f64,
    follow: bool,
    now: f64,

    y_config: YAxisConfig,
    x_config: XAxisConfig,
    smoothing: f64,
}

impl TimeSeriesChart {
    // ── Constructors ────────────────────────────────────────────────

    /// All data upfront. Auto-fits the window to the data extent (with a
    /// small pad) and disables follow mode.
    pub fn new_static(series: Vec<Series>) -> Self {
        let mut buffers: Vec<SeriesBuffer> = series
            .into_iter()
            .enumerate()
            .map(|(i, s)| SeriesBuffer::from_series(s, i))
            .collect();

        let mut t_min = f64::INFINITY;
        let mut t_max = f64::NEG_INFINITY;
        for buf in &buffers {
            if let Some(ext) = buf.time_extent() {
                t_min = t_min.min(ext.start);
                t_max = t_max.max(ext.end);
            }
        }
        if !t_min.is_finite() {
            t_min = 0.0;
            t_max = 1.0;
        }
        let pad = (t_max - t_min) * 0.02;
        let window = TimeRange::new(t_min - pad, t_max + pad);
        let duration = window.duration();

        for buf in &mut buffers {
            buf.mark_loaded(&window);
        }

        Self {
            series: buffers,
            guidelines: Vec::new(),
            window,
            window_duration: duration,
            follow: false,
            now: t_max,
            y_config: YAxisConfig::default(),
            x_config: XAxisConfig::default(),
            smoothing: 0.0,
        }
    }

    /// Window-first, data pushed incrementally. Follow mode on.
    pub fn new(window_duration: f64) -> Self {
        let now = 0.0;
        Self {
            series: Vec::new(),
            guidelines: Vec::new(),
            window: TimeRange::new(now - window_duration, now),
            window_duration,
            follow: true,
            now,
            y_config: YAxisConfig::default(),
            x_config: XAxisConfig::default(),
            smoothing: 0.0,
        }
    }

    // ── Series management ─────────────────────────────────────────────

    /// Add an (initially empty) series; returns its index.
    pub fn add_series(
        &mut self,
        name: &str,
        color: Color,
        style: SeriesStyle,
        connect: ConnectPolicy,
    ) -> usize {
        let idx = self.series.len();
        self.series
            .push(SeriesBuffer::new(name.into(), color, style, connect));
        idx
    }

    /// Push points into a series (sorted + deduplicated by the buffer).
    pub fn push_points(&mut self, series_index: usize, points: &[DataPoint]) {
        if let Some(buf) = self.series.get_mut(series_index) {
            buf.push_points(points);
        }
    }

    /// Mark a time range as loaded for a series (lazy-backfill bookkeeping).
    pub fn mark_loaded(&mut self, series_index: usize, range: &TimeRange) {
        if let Some(buf) = self.series.get_mut(series_index) {
            buf.mark_loaded(range);
        }
    }

    // ── Window control ────────────────────────────────────────────────

    /// Advance "now". In follow mode the window slides to end at `now`.
    pub fn tick(&mut self, now: f64) {
        self.now = now;
        if self.follow {
            self.window = TimeRange::new(now - self.window_duration, now);
        }
    }

    /// Set the visible window duration (seconds), zooming about the
    /// window center (or the live edge in follow mode).
    pub fn set_window_duration(&mut self, seconds: f64) {
        let center = if self.follow {
            self.now
        } else {
            self.window.start + self.window.duration() / 2.0
        };
        self.window_duration = seconds;
        if self.follow {
            self.window = TimeRange::new(self.now - seconds, self.now);
        } else {
            self.window = TimeRange::new(center - seconds / 2.0, center + seconds / 2.0);
        }
    }

    /// Pan the window by `delta_seconds`. Panning away from the live edge
    /// disables follow mode.
    pub fn pan(&mut self, delta_seconds: f64) {
        self.window.start += delta_seconds;
        self.window.end += delta_seconds;
        if self.window.end < self.now {
            self.follow = false;
        }
    }

    /// Zoom by a multiplicative factor (<1 zooms in, >1 zooms out).
    pub fn zoom(&mut self, factor: f64) {
        let new_duration = (self.window_duration * factor).max(1.0);
        self.set_window_duration(new_duration);
    }

    /// Pan by a fraction of the current window width (e.g. `-0.1` =
    /// left by 10%). Convenience for keyboard panning.
    pub fn pan_by_fraction(&mut self, frac: f64) {
        self.pan(frac * self.window_duration);
    }

    /// Pan by a pixel/column delta given the plot's width in cells —
    /// converts the drag distance to seconds. Convenience for mouse-drag
    /// panning (positive `delta_cols` = dragged right = window moves
    /// left, like grabbing the plot).
    pub fn pan_by_columns(&mut self, delta_cols: i32, width: u16) {
        if width == 0 {
            return;
        }
        let time_per_col = self.window_duration / width as f64;
        self.pan(-(delta_cols as f64) * time_per_col);
    }

    /// Re-enable follow mode and snap the window to the live edge.
    pub fn reset_to_live(&mut self) {
        self.follow = true;
        self.window = TimeRange::new(self.now - self.window_duration, self.now);
    }

    /// Time ranges a series still needs loaded to cover the window.
    pub fn needed_ranges(&self, series_index: usize) -> Vec<TimeRange> {
        let Some(buf) = self.series.get(series_index) else {
            return Vec::new();
        };
        if buf.is_loaded(&self.window) {
            return Vec::new();
        }
        vec![self.window]
    }

    /// Whether the window is currently tracking the live edge.
    pub fn is_following(&self) -> bool {
        self.follow
    }

    /// The current visible window width, in seconds.
    pub fn window_duration(&self) -> f64 {
        self.window_duration
    }

    // ── Config ──────────────────────────────────────────────────────

    pub fn set_y_config(&mut self, config: YAxisConfig) {
        self.y_config = config;
    }
    pub fn set_x_config(&mut self, config: XAxisConfig) {
        self.x_config = config;
    }
    /// EMA smoothing factor in `[0, 1]`; 0 disables smoothing.
    pub fn set_smoothing(&mut self, alpha: f64) {
        self.smoothing = alpha.clamp(0.0, 1.0);
    }
    pub fn set_guidelines(&mut self, guidelines: Vec<Guideline>) {
        self.guidelines = guidelines;
    }

    // ── Render ────────────────────────────────────────────────────────

    /// Paint the chart into a canvas `RenderContext`. Coordinates are
    /// canvas-local; the context clips writes to the canvas bounds.
    pub fn paint(&self, ctx: &mut RenderContext<'_>) {
        let w = ctx.width();
        let h = ctx.height();
        if w < 10 || h < 5 {
            return;
        }

        // 1. Collect padded points per series in the current window.
        let collected: Vec<&[DataPoint]> = self
            .series
            .iter()
            .map(|s| s.points_in_range_padded(&self.window))
            .collect();

        let total_points: usize = collected.iter().map(|pts| pts.len()).sum();
        if self.series.is_empty() || total_points == 0 {
            self.paint_empty(ctx, w, h);
            return;
        }

        // 2. Stack transform (identity for now) + optional smoothing.
        let mut stacked: Vec<Vec<StackedPoint>> = collected
            .iter()
            .map(|pts| {
                pts.iter()
                    .map(|p| StackedPoint {
                        timestamp: p.timestamp,
                        top: p.value,
                        baseline: 0.0,
                    })
                    .collect()
            })
            .collect();
        if self.smoothing > 0.0 {
            for s in &mut stacked {
                ema_smooth(s, self.smoothing);
            }
        }

        // 3. Effective Y range.
        let (y_min, y_max) = self.compute_y_range(&stacked);

        // 4. Y-gutter width from the widest tick label.
        let y_fmt = self.y_config.format.unwrap_or(format_y_value);
        let y_ticks = nice_ticks(y_min, y_max, (h as usize / 3).clamp(2, 8));
        let label_w = y_ticks.iter().map(|&v| y_fmt(v).len()).max().unwrap_or(3) as u16 + 2;
        let gutter_w = label_w.max(5).min(w / 3);

        // 5. Local partitions: legend row (top), x-axis row (bottom),
        //    chart in the middle, gutter on the left.
        let legend_h = 1u16;
        let x_axis_h = 1u16;
        let chart_h = h.saturating_sub(legend_h + x_axis_h);
        if chart_h < 2 {
            return;
        }
        let chart_w = w.saturating_sub(gutter_w);
        if chart_w < 4 {
            return;
        }
        let chart_x = gutter_w;
        let chart_y = legend_h;
        let x_axis_y = legend_h + chart_h;

        // 6. Y-axis labels + separators.
        let guideline_rows = self.guideline_rows(y_min, y_max, chart_y, chart_h);
        self.paint_y_axis(
            ctx,
            gutter_w,
            chart_y,
            chart_h,
            &y_ticks,
            y_fmt,
            y_min,
            y_max,
            &guideline_rows,
        );

        // 7. Braille grid for the plot area.
        let mut grid = BrailleGrid::new(chart_w, chart_h);

        // 8. Scale + rasterize series (reverse so series 0 paints on top).
        let scaled: Vec<ScaledSeries> = stacked
            .iter()
            .enumerate()
            .map(|(i, pts)| self.scale_series(pts, &self.series[i], &grid, y_min, y_max))
            .collect();
        for s in scaled.iter().rev() {
            render_series(&mut grid, s);
        }

        // 9. Guideline dots into the grid + labels in the gutter.
        self.paint_guidelines(ctx, &mut grid, y_min, y_max, gutter_w, chart_y);

        // 10. Flush the grid into the chart sub-rect.
        {
            let mut sub = ctx.sub(chart_x, chart_y, chart_w, chart_h);
            grid.paint(&mut sub);
        }

        // 11. X-axis + 12. legend.
        self.paint_x_axis(ctx, chart_x, x_axis_y, chart_w);
        self.paint_legend(ctx, chart_x, chart_w);
    }

    // ── Private rendering helpers ─────────────────────────────────────

    fn paint_empty(&self, ctx: &mut RenderContext<'_>, w: u16, h: u16) {
        let msg = "No data";
        let x = (w / 2).saturating_sub((msg.len() as u16) / 2);
        let y = h / 2;
        ctx.text(x, y, msg, Style::new().fg(MUTED));
    }

    fn compute_y_range(&self, stacked: &[Vec<StackedPoint>]) -> (f64, f64) {
        let mut y_min = f64::INFINITY;
        let mut y_max = f64::NEG_INFINITY;
        for pts in stacked {
            for p in pts {
                if p.top.is_finite() {
                    y_min = y_min.min(p.top);
                    y_max = y_max.max(p.top);
                }
            }
        }

        if let Some(min) = self.y_config.min {
            y_min = min;
        }
        if let Some(max) = self.y_config.max {
            y_max = max;
        }

        if !y_min.is_finite() || !y_max.is_finite() {
            return (0.0, 1.0);
        }
        if (y_max - y_min).abs() < 1e-12 {
            let pad = if y_min.abs() < 1e-12 {
                1.0
            } else {
                y_min.abs() * 0.1
            };
            y_min -= pad;
            y_max += pad;
        } else {
            let pad = (y_max - y_min) * 0.05;
            if self.y_config.min.is_none() {
                y_min -= pad;
            }
            if self.y_config.max.is_none() {
                y_max += pad;
            }
        }
        (y_min, y_max)
    }

    fn scale_series(
        &self,
        stacked_pts: &[StackedPoint],
        buf: &SeriesBuffer,
        grid: &BrailleGrid,
        y_min: f64,
        y_max: f64,
    ) -> ScaledSeries {
        let bw = grid.braille_width() as f64;
        let bh = grid.braille_height() as f64;
        let x_range = self.window.duration();
        let y_range = y_max - y_min;

        let points = stacked_pts
            .iter()
            .map(|p| {
                if !p.top.is_finite() {
                    return None;
                }
                let t_frac = if x_range > 0.0 {
                    (p.timestamp - self.window.start) / x_range
                } else {
                    0.5
                };
                let v_frac = if y_range > 0.0 {
                    (p.top - y_min) / y_range
                } else {
                    0.5
                };
                let b_frac = if y_range > 0.0 {
                    (p.baseline - y_min) / y_range
                } else {
                    0.0
                };
                Some(ScaledPoint {
                    bx: (t_frac * (bw - 1.0)).round() as i32,
                    top_by: ((1.0 - v_frac) * (bh - 1.0)).round() as i32,
                    baseline_by: ((1.0 - b_frac) * (bh - 1.0)).round() as i32,
                })
            })
            .collect();

        ScaledSeries {
            points,
            color: buf.color,
            style: buf.style,
            connect: buf.connect,
        }
    }

    /// Local chart rows occupied by guideline labels (collision avoidance).
    fn guideline_rows(&self, y_min: f64, y_max: f64, chart_y: u16, chart_h: u16) -> Vec<u16> {
        let y_range = y_max - y_min;
        if y_range <= 0.0 {
            return Vec::new();
        }
        self.guidelines
            .iter()
            .filter(|gl| gl.label.is_some())
            .filter_map(|gl| {
                let frac = (gl.y_value - y_min) / y_range;
                if !(0.0..=1.0).contains(&frac) {
                    return None;
                }
                let row = ((1.0 - frac) * chart_h.saturating_sub(1) as f64).round() as u16;
                Some(chart_y + row)
            })
            .collect()
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_y_axis(
        &self,
        ctx: &mut RenderContext<'_>,
        gutter_w: u16,
        chart_y: u16,
        chart_h: u16,
        ticks: &[f64],
        fmt: fn(f64) -> String,
        y_min: f64,
        y_max: f64,
        guideline_rows: &[u16],
    ) {
        let y_range = y_max - y_min;
        let label_style = Style::new().fg(LABEL);
        let sep_style = Style::new().fg(MUTED);
        let sep_x = gutter_w.saturating_sub(1);

        // Vertical separator down the whole chart band.
        for y in chart_y..chart_y + chart_h {
            ctx.set(sep_x, y, '\u{2502}', sep_style);
        }

        if y_range <= 0.0 {
            return;
        }
        for &tick_val in ticks {
            let frac = (tick_val - y_min) / y_range;
            let row = ((1.0 - frac) * chart_h.saturating_sub(1) as f64).round() as u16;
            let y = chart_y + row;
            if y < chart_y || y >= chart_y + chart_h {
                continue;
            }

            // Tick mark on the separator either way.
            ctx.set(sep_x, y, '\u{251c}', sep_style);

            // Skip the value label if a guideline label owns this row.
            if guideline_rows.contains(&y) {
                continue;
            }
            let label = fmt(tick_val);
            let label_len = label.len() as u16;
            let x = if label_len + 1 < gutter_w {
                gutter_w - 2 - label_len
            } else {
                0
            };
            ctx.text(x, y, &label, label_style);
        }
    }

    fn paint_x_axis(&self, ctx: &mut RenderContext<'_>, chart_x: u16, y: u16, chart_w: u16) {
        if chart_w == 0 {
            return;
        }
        let label_style = Style::new().fg(LABEL);
        let range_dur = self.window.duration();
        let x_fmt: Box<dyn Fn(f64) -> String> = match self.x_config.format {
            Some(f) => Box::new(f),
            None => Box::new(move |t| format_timestamp(t, range_dur)),
        };

        let target_ticks = (chart_w as usize / 8).clamp(2, 10);
        let ticks = nice_ticks(self.window.start, self.window.end, target_ticks);

        for &tick_val in &ticks {
            let frac = if range_dur > 0.0 {
                (tick_val - self.window.start) / range_dur
            } else {
                0.5
            };
            let col = (frac * chart_w.saturating_sub(1) as f64).round() as i32;
            let label = (*x_fmt)(tick_val);
            let label_chars: Vec<char> = label.chars().collect();
            let label_len = label_chars.len() as i32;
            // Center the label on the tick, in chart-local columns.
            let start_col = col - label_len / 2;
            let left = 0i32;
            let right = chart_w as i32;

            let clip_left = (left - start_col).max(0) as usize;
            let clip_right = ((start_col + label_len) - right).max(0) as usize;
            if clip_left >= label_chars.len() || clip_right >= label_chars.len() {
                continue;
            }
            let vis_start = clip_left;
            let vis_end = label_chars.len() - clip_right;
            if vis_start >= vis_end {
                continue;
            }
            for (offset, &ch) in label_chars[vis_start..vis_end].iter().enumerate() {
                let i = vis_start + offset;
                let local_col = (start_col + i as i32) as u16;
                let edge_clipped =
                    (clip_left > 0 && i == vis_start) || (clip_right > 0 && i == vis_end - 1);
                let display = if edge_clipped { '\u{2026}' } else { ch };
                ctx.set(chart_x + local_col, y, display, label_style);
            }
        }
    }

    fn paint_legend(&self, ctx: &mut RenderContext<'_>, chart_x: u16, chart_w: u16) {
        if chart_w == 0 || self.series.is_empty() {
            return;
        }
        let muted = Style::new().fg(MUTED);
        let mut x = chart_x + 1;
        let right = chart_x + chart_w;
        let y = 0;

        for s in &self.series {
            if x + 4 >= right {
                break;
            }
            ctx.set(x, y, '\u{2022}', Style::new().fg(s.color));
            x += 2; // bullet + space
            for ch in s.name.chars() {
                if x >= right {
                    break;
                }
                ctx.set(x, y, ch, muted);
                x += 1;
            }
            x += 2; // gap between entries
        }
    }

    fn paint_guidelines(
        &self,
        ctx: &mut RenderContext<'_>,
        grid: &mut BrailleGrid,
        y_min: f64,
        y_max: f64,
        gutter_w: u16,
        chart_y: u16,
    ) {
        let y_range = y_max - y_min;
        if y_range <= 0.0 {
            return;
        }
        let bh = grid.braille_height() as f64;
        let bw = grid.braille_width();
        let sep_x = gutter_w.saturating_sub(1);

        for gl in &self.guidelines {
            let frac = (gl.y_value - y_min) / y_range;
            if !(0.0..=1.0).contains(&frac) {
                continue;
            }
            let by = ((1.0 - frac) * (bh - 1.0)).round() as i32;
            // Dashed horizontal line in the grid.
            for bx in (0..bw).step_by(2) {
                grid.set_dot(bx, by, gl.color);
            }
            if let Some(label) = &gl.label {
                let chart_row = (by / 4) as u16;
                let y = chart_y + chart_row;
                let start_x = sep_x.saturating_sub(label.len() as u16 + 1);
                ctx.text(start_x, y, label, Style::new().fg(gl.color));
            }
        }
    }
}

/// A shareable handle that owns a [`TimeSeriesChart`] and renders it onto
/// a `<canvas>` element.
///
/// Cloning shares the same chart. Mutate via [`with`](Self::with) (e.g.
/// to push streaming data or zoom), then ask the runtime to repaint.
#[derive(Clone)]
pub struct TimeSeriesView {
    inner: Rc<RefCell<TimeSeriesChart>>,
}

impl TimeSeriesView {
    pub fn new(chart: TimeSeriesChart) -> Self {
        Self {
            inner: Rc::new(RefCell::new(chart)),
        }
    }

    /// Create a `<canvas>` element wired to paint this chart and return
    /// its `NodeId`. The caller appends it into the tree and sizes it
    /// (e.g. `dom.node_mut(id).set_width(Size::Flex(1))`).
    pub fn mount(&self, dom: &mut TuiDom) -> NodeId {
        let id = dom.create_element("canvas");
        let inner = self.inner.clone();
        canvas::set_paint(dom, id, move |_dom, ctx| {
            inner.borrow().paint(ctx);
        });
        id
    }

    /// Borrow the chart mutably to update it (push data, tick, zoom…).
    pub fn with<R>(&self, f: impl FnOnce(&mut TimeSeriesChart) -> R) -> R {
        f(&mut self.inner.borrow_mut())
    }

    /// Wire keyboard + mouse interaction onto the mounted `<canvas>`:
    ///
    /// - `+` / `-` (or `=`) — zoom in / out
    /// - `h` / `l` or `←` / `→` — pan left / right
    /// - `0` — reset to the live edge (re-enable follow)
    /// - mouse wheel — zoom
    /// - left-drag — pan
    ///
    /// Each handler mutates the shared chart and calls
    /// `ctx.request_redraw()` (the chart's state lives outside the DOM,
    /// so the mutation tracker can't see it — this is exactly the
    /// `EventCtx::request_redraw` affordance, rdom 0.3+). The canvas is
    /// made focusable (`tabindex="0"`) so it can receive keyboard events;
    /// focus it (click, Tab, or `dom`-level focus) for the keys to fire.
    pub fn install_interaction(&self, dom: &mut TuiDom, canvas: NodeId) {
        let _ = dom.node_mut(canvas).set_attribute("tabindex", "0");

        // Keyboard: zoom / pan / reset.
        let kb = self.inner.clone();
        dom.add_event_listener(canvas, "keydown", ListenerOptions::default(), move |ctx| {
            let Some(key) = ctx.event.detail.as_keyboard() else {
                return;
            };
            let mut handled = true;
            {
                let mut c = kb.borrow_mut();
                match key.key.as_str() {
                    "+" | "=" => c.zoom(0.5),
                    "-" => c.zoom(2.0),
                    "h" | "ArrowLeft" => c.pan_by_fraction(-0.1),
                    "l" | "ArrowRight" => c.pan_by_fraction(0.1),
                    "0" => c.reset_to_live(),
                    _ => handled = false,
                }
            }
            if handled {
                ctx.event.prevent_default();
                ctx.request_redraw();
            }
        })
        .expect("keydown listener");

        // Wheel: zoom (up = in, down = out).
        let wheel = self.inner.clone();
        dom.add_event_listener(canvas, "wheel", ListenerOptions::default(), move |ctx| {
            let Some(m) = ctx.event.detail.as_mouse() else {
                return;
            };
            let factor = if m.delta_y < 0 { 0.8 } else { 1.25 };
            wheel.borrow_mut().zoom(factor);
            ctx.event.prevent_default();
            ctx.request_redraw();
        })
        .expect("wheel listener");

        // Left-drag: pan. `mousedown` records the anchor column,
        // `mousemove` pans by the delta (only while dragging), `mouseup`
        // ends the gesture.
        let drag: Rc<std::cell::Cell<Option<i32>>> = Rc::new(std::cell::Cell::new(None));
        let down = drag.clone();
        dom.add_event_listener(
            canvas,
            "mousedown",
            ListenerOptions::default(),
            move |ctx| {
                if let Some(m) = ctx.event.detail.as_mouse() {
                    down.set(Some(m.client_x));
                }
            },
        )
        .expect("mousedown listener");

        let moving = drag.clone();
        let pan = self.inner.clone();
        dom.add_event_listener(
            canvas,
            "mousemove",
            ListenerOptions::default(),
            move |ctx| {
                let Some(start) = moving.get() else {
                    return;
                };
                let Some(m) = ctx.event.detail.as_mouse() else {
                    return;
                };
                let dx = m.client_x - start;
                if dx == 0 {
                    return;
                }
                let width = ctx
                    .dom
                    .node(canvas)
                    .content_layout_rect()
                    .map(|r| r.width)
                    .unwrap_or(0);
                pan.borrow_mut().pan_by_columns(dx, width);
                moving.set(Some(m.client_x));
                ctx.request_redraw();
            },
        )
        .expect("mousemove listener");

        let up = drag.clone();
        dom.add_event_listener(canvas, "mouseup", ListenerOptions::default(), move |_ctx| {
            up.set(None);
        })
        .expect("mouseup listener");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::series_color;

    fn sine_series(name: &str, n: usize) -> Series {
        Series::line(
            name,
            (0..n)
                .map(|i| DataPoint::new(i as f64 * 60.0, (i as f64 * 0.3).sin() * 50.0 + 50.0))
                .collect(),
        )
    }

    #[test]
    fn tick_advances_follow_window() {
        let mut c = TimeSeriesChart::new(60.0);
        c.tick(100.0);
        assert_eq!(c.window.start, 40.0);
        assert_eq!(c.window.end, 100.0);
        c.tick(110.0);
        assert_eq!(c.window.start, 50.0);
        assert_eq!(c.window.end, 110.0);
    }

    #[test]
    fn pan_disables_follow() {
        let mut c = TimeSeriesChart::new(60.0);
        c.tick(100.0);
        assert!(c.is_following());
        c.pan(-20.0);
        assert!(!c.is_following());
    }

    #[test]
    fn reset_to_live_reenables_follow() {
        let mut c = TimeSeriesChart::new(60.0);
        c.tick(100.0);
        c.pan(-20.0);
        c.reset_to_live();
        assert!(c.is_following());
        assert_eq!(c.window.end, 100.0);
    }

    #[test]
    fn zoom_about_center_when_paused() {
        let mut c = TimeSeriesChart::new(60.0);
        c.tick(100.0);
        c.pan(-10.0); // pauses follow, window 30..90
        c.set_window_duration(30.0);
        assert!((c.window.start - 45.0).abs() < 0.01);
        assert!((c.window.end - 75.0).abs() < 0.01);
    }

    #[test]
    fn new_static_auto_ranges_window() {
        let c = TimeSeriesChart::new_static(vec![Series::line(
            "t",
            vec![DataPoint::new(10.0, 5.0), DataPoint::new(20.0, 15.0)],
        )]);
        assert!(!c.is_following());
        assert!(c.window.start <= 10.0);
        assert!(c.window.end >= 20.0);
    }

    #[test]
    fn needed_ranges_empty_when_loaded() {
        let c = TimeSeriesChart::new_static(vec![Series::line(
            "t",
            vec![DataPoint::new(10.0, 5.0), DataPoint::new(20.0, 15.0)],
        )]);
        assert!(c.needed_ranges(0).is_empty());
    }

    #[test]
    fn needed_ranges_returns_window_when_unloaded() {
        let mut c = TimeSeriesChart::new(60.0);
        c.add_series("t", series_color(0), SeriesStyle::Line, ConnectPolicy::Gap);
        c.tick(100.0);
        let needed = c.needed_ranges(0);
        assert_eq!(needed.len(), 1);
        assert_eq!(needed[0].start, 40.0);
        assert_eq!(needed[0].end, 100.0);
    }

    #[test]
    fn scale_series_maps_into_grid_bounds() {
        let c = TimeSeriesChart::new_static(vec![sine_series("a", 20)]);
        let grid = BrailleGrid::new(40, 10);
        let stacked: Vec<StackedPoint> = (0..20)
            .map(|i| StackedPoint {
                timestamp: i as f64 * 60.0,
                top: (i as f64 * 0.3).sin() * 50.0 + 50.0,
                baseline: 0.0,
            })
            .collect();
        let scaled = c.scale_series(&stacked, &c.series[0], &grid, 0.0, 100.0);
        for p in scaled.points.iter().flatten() {
            assert!(p.bx >= 0 && p.bx < grid.braille_width());
            assert!(p.top_by >= 0 && p.top_by < grid.braille_height());
        }
    }
}
