//! Color palette for data-visualization components.
//!
//! rdom-charts is theme-agnostic: it speaks `rdom_tui::Color` directly.
//! Callers who want their own colors pass explicit `Color`s; callers who
//! don't get an auto-assigned series color from [`series_color`].
//!
//! These defaults are reasonable on both dark and light terminals but
//! are intentionally plain — a downstream app that has a real theme
//! should pass its own colors rather than rely on these.

use rdom_tui::Color;

/// Default series-color assignment order. Index wraps around.
///
/// Picked for distinguishability on a dark background.
pub const SERIES_PALETTE: &[Color] = &[
    Color::Rgb(0x4f, 0x9d, 0xff), // brand / blue
    Color::Rgb(0x3f, 0xc9, 0x6b), // success / green
    Color::Rgb(0xff, 0x9f, 0x40), // orange
    Color::Rgb(0xb4, 0x7c, 0xff), // purple
    Color::Rgb(0x36, 0xd0, 0xd8), // cyan
    Color::Rgb(0xff, 0x6b, 0xc2), // pink
    Color::Rgb(0xa6, 0xe2, 0x2e), // lime
    Color::Rgb(0x7a, 0x86, 0xff), // indigo
    Color::Rgb(0xf5, 0xc2, 0x42), // warning / yellow
    Color::Rgb(0xff, 0x5c, 0x5c), // danger / red
];

/// Auto-assign a series color by index, wrapping around the palette.
pub fn series_color(index: usize) -> Color {
    SERIES_PALETTE[index % SERIES_PALETTE.len()]
}

/// Muted color for axis gridlines, ticks, and inactive chrome.
pub const MUTED: Color = Color::Rgb(0x6b, 0x72, 0x80);

/// Foreground color for axis labels and legend text.
pub const LABEL: Color = Color::Rgb(0x9c, 0xa3, 0xaf);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn series_color_wraps() {
        assert_eq!(series_color(0), SERIES_PALETTE[0]);
        assert_eq!(series_color(1), SERIES_PALETTE[1]);
        assert_eq!(series_color(SERIES_PALETTE.len()), SERIES_PALETTE[0]);
        assert_eq!(series_color(SERIES_PALETTE.len() + 2), SERIES_PALETTE[2]);
    }
}
