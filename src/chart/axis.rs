//! Axis utilities: "nice" tick computation and value/time formatting.
//!
//! Ported unchanged from the upstream lens component — pure numeric
//! helpers with no rendering or theme coupling.

/// Compute "nice" tick values across `[min, max]`, aiming for roughly
/// `target_count` ticks landing on 1/2/5×10ⁿ boundaries.
pub fn nice_ticks(min: f64, max: f64, target_count: usize) -> Vec<f64> {
    let range = max - min;
    if !range.is_finite() || range <= 0.0 || target_count == 0 {
        return if min.is_finite() { vec![min] } else { vec![] };
    }
    let rough_step = range / target_count as f64;
    let magnitude = 10f64.powf(rough_step.log10().floor());
    let residual = rough_step / magnitude;
    let nice_step = if residual <= 1.5 {
        magnitude
    } else if residual <= 3.0 {
        2.0 * magnitude
    } else if residual <= 7.0 {
        5.0 * magnitude
    } else {
        10.0 * magnitude
    };
    let start = (min / nice_step).ceil() * nice_step;
    let mut ticks = Vec::new();
    let mut v = start;
    for _ in 0..100 {
        if v > max + nice_step * 0.01 {
            break;
        }
        ticks.push(v);
        v += nice_step;
    }
    ticks
}

/// Default Y-axis label formatter (auto-scales K/M/G).
pub fn format_y_value(v: f64) -> String {
    let abs = v.abs();
    if abs >= 1_000_000_000.0 {
        format!("{:.1}G", v / 1_000_000_000.0)
    } else if abs >= 1_000_000.0 {
        format!("{:.1}M", v / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{:.1}K", v / 1_000.0)
    } else if abs >= 100.0 {
        format!("{:.0}", v)
    } else if abs >= 10.0 {
        format!("{:.1}", v)
    } else if abs >= 1.0 {
        format!("{:.2}", v)
    } else {
        format!("{:.3}", v)
    }
}

/// Default X-axis time formatter. `range_duration` (seconds) selects
/// between a `MM/DD` (multi-day) and `HH:MM` (intraday) presentation.
pub fn format_timestamp(t: f64, range_duration: f64) -> String {
    let secs = t as i64;
    if range_duration > 86400.0 * 2.0 {
        let days = secs / 86400;
        let month = ((days % 365) / 30) + 1;
        let day = ((days % 365) % 30) + 1;
        format!("{month:02}/{day:02}")
    } else {
        let h = (secs % 86400) / 3600;
        let m = (secs % 3600) / 60;
        format!("{h:02}:{m:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nice_ticks_basic() {
        let ticks = nice_ticks(0.0, 100.0, 5);
        assert!(!ticks.is_empty());
        for &t in &ticks {
            assert!((0.0..=100.0).contains(&t));
            assert_eq!(t % 10.0, 0.0, "tick {t} not a multiple of 10");
        }
    }

    #[test]
    fn nice_ticks_small_range() {
        let ticks = nice_ticks(0.0, 1.0, 5);
        assert!(!ticks.is_empty());
        for &t in &ticks {
            assert!((0.0..=1.0).contains(&t));
        }
    }

    #[test]
    fn nice_ticks_degenerate() {
        assert_eq!(nice_ticks(5.0, 5.0, 5), vec![5.0]);
        assert!(nice_ticks(f64::NAN, f64::NAN, 5).is_empty());
    }

    #[test]
    fn format_y_scales() {
        assert_eq!(format_y_value(1500.0), "1.5K");
        assert_eq!(format_y_value(2_500_000.0), "2.5M");
        assert_eq!(format_y_value(50.0), "50.0");
    }

    #[test]
    fn format_timestamp_intraday_vs_multiday() {
        // 1 hour window → HH:MM
        let intraday = format_timestamp(3661.0, 3600.0);
        assert_eq!(intraday, "01:01");
        // 5 day window → MM/DD
        let multiday = format_timestamp(86400.0, 86400.0 * 5.0);
        assert!(multiday.contains('/'));
    }
}
