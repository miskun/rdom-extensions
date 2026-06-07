//! `BrailleGrid`: a 2×4-resolution dot buffer for sub-cell line drawing.
//!
//! Each terminal cell is a 2×4 braille dot matrix (8 dots, U+2800..U+28FF),
//! giving 2× horizontal and 4× vertical resolution over plain cells. Lines
//! are rasterized with Bresenham in dot space, then flushed to a canvas
//! [`RenderContext`] one braille glyph per cell.

use rdom_tui::Color;
use rdom_tui::Style;
use rdom_tui::runtime::builtins::canvas::RenderContext;

use super::data::{ConnectPolicy, SeriesStyle};
use crate::palette::MUTED;

pub(crate) const BRAILLE_BASE: u32 = 0x2800;

/// Map sub-cell coordinate (`sx`: 0..2, `sy`: 0..4) to its braille bit.
pub(crate) fn dot_bit(sx: u8, sy: u8) -> u8 {
    match (sx, sy) {
        (0, 0) => 0,
        (0, 1) => 1,
        (0, 2) => 2,
        (0, 3) => 6,
        (1, 0) => 3,
        (1, 1) => 4,
        (1, 2) => 5,
        (1, 3) => 7,
        _ => 0,
    }
}

/// Braille dot grid. One `u8` bitmask + one color per terminal cell.
pub(crate) struct BrailleGrid {
    pub cell_width: u16,
    pub cell_height: u16,
    pub dots: Vec<u8>,
    pub colors: Vec<Color>,
}

impl BrailleGrid {
    pub fn new(cell_width: u16, cell_height: u16) -> Self {
        let len = cell_width as usize * cell_height as usize;
        Self {
            cell_width,
            cell_height,
            dots: vec![0; len],
            colors: vec![MUTED; len],
        }
    }

    pub fn braille_width(&self) -> i32 {
        self.cell_width as i32 * 2
    }

    pub fn braille_height(&self) -> i32 {
        self.cell_height as i32 * 4
    }

    /// Set a single braille dot. Bounds-checked, silent on out-of-range.
    pub fn set_dot(&mut self, bx: i32, by: i32, color: Color) {
        if bx < 0 || by < 0 || bx >= self.braille_width() || by >= self.braille_height() {
            return;
        }
        let cx = (bx / 2) as usize;
        let cy = (by / 4) as usize;
        let sx = (bx % 2) as u8;
        let sy = (by % 4) as u8;
        let idx = cy * self.cell_width as usize + cx;
        if idx < self.dots.len() {
            self.dots[idx] |= 1 << dot_bit(sx, sy);
            self.colors[idx] = color;
        }
    }

    /// Bresenham line drawing in braille dot space.
    pub fn draw_line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, color: Color) {
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        let mut x = x0;
        let mut y = y0;
        loop {
            self.set_dot(x, y, color);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                if x == x1 {
                    break;
                }
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                if y == y1 {
                    break;
                }
                err += dx;
                y += sy;
            }
        }
    }

    /// Flush the grid into a canvas `RenderContext` at local `(0, 0)`.
    /// Empty cells are skipped so the canvas background shows through.
    pub fn paint(&self, ctx: &mut RenderContext<'_>) {
        for cy in 0..self.cell_height {
            for cx in 0..self.cell_width {
                let idx = cy as usize * self.cell_width as usize + cx as usize;
                let bits = self.dots[idx];
                if bits == 0 {
                    continue;
                }
                let ch = char::from_u32(BRAILLE_BASE | bits as u32).unwrap_or(' ');
                ctx.set(cx, cy, ch, Style::new().fg(self.colors[idx]));
            }
        }
    }
}

// ── Render pipeline types ──────────────────────────────────────────

/// A data point after stacking: a value at a timestamp.
///
/// (When stacked-area / fill rendering lands it will carry a `baseline`
/// here; it's omitted until a renderer consumes it, per the crate's
/// no-dead-scaffolding rule.)
pub(crate) struct StackedPoint {
    pub timestamp: f64,
    pub top: f64,
}

/// A point scaled to braille coordinates.
pub(crate) struct ScaledPoint {
    pub bx: i32,
    pub top_by: i32,
}

/// A series scaled and ready to rasterize.
pub(crate) struct ScaledSeries {
    pub points: Vec<Option<ScaledPoint>>,
    pub color: Color,
    pub style: SeriesStyle,
    pub connect: ConnectPolicy,
}

/// Forward-backward EMA smoothing (zero phase shift).
pub(crate) fn ema_smooth(points: &mut [StackedPoint], alpha: f64) {
    if alpha <= 0.0 || points.len() < 2 {
        return;
    }
    let mut ema = points[0].top;
    for p in points.iter_mut() {
        if p.top.is_finite() {
            ema = alpha * p.top + (1.0 - alpha) * ema;
            p.top = ema;
        }
    }
    ema = points.last().unwrap().top;
    for p in points.iter_mut().rev() {
        if p.top.is_finite() {
            ema = alpha * p.top + (1.0 - alpha) * ema;
            p.top = ema;
        }
    }
}

pub(crate) fn render_series(grid: &mut BrailleGrid, series: &ScaledSeries) {
    match series.style {
        SeriesStyle::Line => render_line(grid, series),
    }
}

fn render_line(grid: &mut BrailleGrid, series: &ScaledSeries) {
    let mut prev: Option<&ScaledPoint> = None;
    for pt in &series.points {
        match pt {
            Some(p) => {
                grid.set_dot(p.bx, p.top_by, series.color);
                if let Some(prev_p) = prev {
                    grid.draw_line(prev_p.bx, prev_p.top_by, p.bx, p.top_by, series.color);
                }
                prev = Some(p);
            }
            None => match series.connect {
                ConnectPolicy::Gap => prev = None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::series_color;

    #[test]
    fn dot_bits_match_braille_layout() {
        assert_eq!(dot_bit(0, 0), 0);
        assert_eq!(dot_bit(0, 3), 6);
        assert_eq!(dot_bit(1, 0), 3);
        assert_eq!(dot_bit(1, 3), 7);
    }

    #[test]
    fn all_dots_is_full_glyph() {
        let all_bits: u8 = (0..4)
            .flat_map(|sy| (0..2).map(move |sx| 1u8 << dot_bit(sx, sy)))
            .fold(0u8, |acc, b| acc | b);
        assert_eq!(all_bits, 0xFF);
        assert_eq!(
            char::from_u32(BRAILLE_BASE | all_bits as u32).unwrap(),
            '\u{28FF}'
        );
    }

    #[test]
    fn set_dot_sets_correct_bit() {
        let mut grid = BrailleGrid::new(2, 2);
        assert_eq!(grid.braille_width(), 4);
        assert_eq!(grid.braille_height(), 8);
        grid.set_dot(0, 0, series_color(0));
        assert_eq!(grid.dots[0], 1 << dot_bit(0, 0));
        grid.set_dot(1, 0, series_color(1));
        assert_eq!(grid.dots[0], (1 << dot_bit(0, 0)) | (1 << dot_bit(1, 0)));
        assert_eq!(grid.colors[0], series_color(1));
    }

    #[test]
    fn set_dot_out_of_bounds_is_noop() {
        let mut grid = BrailleGrid::new(2, 2);
        grid.set_dot(-1, 0, series_color(0));
        grid.set_dot(0, -1, series_color(0));
        grid.set_dot(100, 0, series_color(0));
        grid.set_dot(0, 100, series_color(0));
        assert!(grid.dots.iter().all(|&d| d == 0));
    }

    #[test]
    fn horizontal_line_sets_row() {
        let mut grid = BrailleGrid::new(4, 1);
        grid.draw_line(0, 2, 7, 2, series_color(0));
        for bx in 0..8 {
            let cx = bx / 2;
            let sx = (bx % 2) as u8;
            assert!(
                grid.dots[cx] & (1u8 << dot_bit(sx, 2)) != 0,
                "missing dot at bx={bx}"
            );
        }
    }

    #[test]
    fn diagonal_line_is_continuous() {
        let mut grid = BrailleGrid::new(4, 2);
        grid.draw_line(0, 0, 7, 7, series_color(0));
        let total: u32 = grid.dots.iter().map(|&b| b.count_ones()).sum();
        assert!(total >= 8, "expected continuous line, got {total} dots");
    }

    #[test]
    fn ema_reduces_variance() {
        let mut points: Vec<StackedPoint> = (0..10)
            .map(|i| StackedPoint {
                timestamp: i as f64,
                top: if i % 2 == 0 { 100.0 } else { 0.0 },
            })
            .collect();
        ema_smooth(&mut points, 0.3);
        for p in &points {
            assert!(p.top > 20.0 && p.top < 80.0, "value {} not near 50", p.top);
        }
    }

    #[test]
    fn ema_zero_alpha_passthrough() {
        let mut points = vec![
            StackedPoint {
                timestamp: 0.0,
                top: 10.0,
            },
            StackedPoint {
                timestamp: 1.0,
                top: 90.0,
            },
        ];
        ema_smooth(&mut points, 0.0);
        assert_eq!(points[0].top, 10.0);
        assert_eq!(points[1].top, 90.0);
    }
}
