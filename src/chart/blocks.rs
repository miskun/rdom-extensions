//! Block-fill helpers for bar-style charts.
//!
//! Horizontal bars get sub-cell precision from the Unicode "left
//! block" eighths (U+2588 full … U+258F one-eighth): a bar of fractional
//! width renders as N full blocks plus one partial block, padded with
//! spaces to a fixed column width.

/// `H_EIGHTHS[n]` is the glyph filling `n`/8 of a cell from the left.
/// Index 0 is a space (empty); index 8 is a full block.
pub(crate) const H_EIGHTHS: [char; 9] = [
    ' ',        // 0
    '\u{258F}', // 1 ▏
    '\u{258E}', // 2 ▎
    '\u{258D}', // 3 ▍
    '\u{258C}', // 4 ▌
    '\u{258B}', // 5 ▋
    '\u{258A}', // 6 ▊
    '\u{2589}', // 7 ▉
    '\u{2588}', // 8 █
];

/// Render a horizontal bar of `ratio` (clamped to `[0,1]`) across `width`
/// cells. Returns a string of exactly `width` display columns: full
/// blocks, at most one partial block, then spaces.
pub(crate) fn h_bar(width: u16, ratio: f64) -> String {
    let ratio = ratio.clamp(0.0, 1.0);
    let total_eighths = (ratio * width as f64 * 8.0).round() as u32;
    let full = ((total_eighths / 8) as u16).min(width);
    let rem = (total_eighths % 8) as usize;

    let mut s = String::with_capacity(width as usize);
    let mut cols = 0u16;
    for _ in 0..full {
        s.push('\u{2588}');
        cols += 1;
    }
    if cols < width && rem > 0 {
        s.push(H_EIGHTHS[rem]);
        cols += 1;
    }
    while cols < width {
        s.push(' ');
        cols += 1;
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cols(s: &str) -> usize {
        s.chars().count()
    }

    #[test]
    fn full_ratio_fills_all_cells() {
        let s = h_bar(8, 1.0);
        assert_eq!(cols(&s), 8);
        assert!(s.chars().all(|c| c == '\u{2588}'));
    }

    #[test]
    fn zero_ratio_is_all_spaces() {
        let s = h_bar(8, 0.0);
        assert_eq!(cols(&s), 8);
        assert!(s.chars().all(|c| c == ' '));
    }

    #[test]
    fn half_of_eight_is_four_full_blocks() {
        let s = h_bar(8, 0.5);
        assert_eq!(cols(&s), 8);
        assert_eq!(s.chars().filter(|&c| c == '\u{2588}').count(), 4);
        assert_eq!(s.chars().filter(|&c| c == ' ').count(), 4);
    }

    #[test]
    fn fractional_uses_a_partial_block() {
        // ratio puts 1 full + ~half a cell → a partial glyph appears.
        let s = h_bar(4, 0.375); // 0.375*4*8 = 12 eighths → 1 full + 4 eighths (▌)
        assert_eq!(cols(&s), 4);
        assert_eq!(s.chars().filter(|&c| c == '\u{2588}').count(), 1);
        assert!(
            s.contains('\u{258C}'),
            "expected a half-block partial: {s:?}"
        );
    }

    #[test]
    fn always_exactly_width_columns() {
        for w in 1..20u16 {
            for r in [0.0, 0.1, 0.33, 0.5, 0.9, 1.0, 1.5, -0.2] {
                assert_eq!(cols(&h_bar(w, r)), w as usize, "w={w} r={r}");
            }
        }
    }
}
