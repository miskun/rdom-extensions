//! Headless coverage for the time-series demo variants — in particular
//! that **threshold/guideline lines** render with their colored labels
//! (the `100ms` / `150ms` reference lines), plus smoothing and the empty
//! state.

use rdom_charts::{DataPoint, Guideline, Series, TimeSeriesChart, TimeSeriesView};
use rdom_tui::render::{Buffer, LayoutExt, PaintExt, Rect};
use rdom_tui::style::{CascadeExt, Stylesheet};
use rdom_tui::{Color, Size, TuiDom, TuiNodeMutExt, TuiStyle};

fn render(view: &TimeSeriesView, w: u16, h: u16) -> String {
    let mut dom = TuiDom::new();
    let root = dom.root();
    let canvas = view.mount(&mut dom);
    dom.node_mut(canvas)
        .set_inline_style(TuiStyle::new().width(Size::Fixed(w)).height(Size::Fixed(h)));
    dom.append_child(root, canvas).unwrap();

    let vp = Rect::new(0, 0, w, h);
    dom.cascade(&Stylesheet::new());
    dom.layout_dom(vp);
    let mut buf = Buffer::empty(vp);
    dom.paint_dom(&mut buf, vp);

    let mut s = String::new();
    for y in 0..h {
        for x in 0..w {
            if let Some(c) = buf.cell(x, y) {
                if !c.is_spacer() {
                    s.push_str(c.symbol());
                }
            }
        }
        s.push('\n');
    }
    s
}

fn braille_count(s: &str) -> usize {
    s.chars()
        .filter(|&c| ('\u{2800}'..='\u{28FF}').contains(&c))
        .count()
}

/// "Spiky": two series + two threshold lines at 100ms and 150ms with
/// different colors. Pins that guideline labels render.
#[test]
fn spiky_demo_renders_threshold_lines() {
    let latency: Vec<DataPoint> = (0..200)
        .map(|i| {
            let f = i as f64;
            let base = 50.0 + 20.0 * (f * 0.05).sin();
            let spike = if (f * 7.3).sin() > 0.85 { 80.0 } else { 0.0 };
            DataPoint::new(f * 15.0, (base + spike).clamp(5.0, 200.0))
        })
        .collect();
    let mut chart = TimeSeriesChart::new_static(vec![Series::line("Latency (ms)", latency)]);
    chart.set_guidelines(vec![
        Guideline {
            y_value: 100.0,
            color: Color::Rgb(0xf5, 0xc2, 0x42),
            label: Some("100ms".into()),
        },
        Guideline {
            y_value: 150.0,
            color: Color::Rgb(0xff, 0x5c, 0x5c),
            label: Some("150ms".into()),
        },
    ]);
    let out = render(&TimeSeriesView::new(chart), 80, 22);

    assert!(
        braille_count(&out) > 20,
        "the latency line should rasterize"
    );
    assert!(
        out.contains("100ms"),
        "100ms threshold label must render:\n{out}"
    );
    assert!(
        out.contains("150ms"),
        "150ms threshold label must render:\n{out}"
    );
}

/// "Smooth": multi-series + EMA smoothing + a single 80% guideline +
/// percent-formatted Y axis.
#[test]
fn smooth_demo_renders() {
    use rdom_charts::YAxisConfig;
    let mk = |freq: f64, off: f64| -> Vec<DataPoint> {
        (0..120)
            .map(|i| {
                let f = i as f64;
                DataPoint::new(f * 30.0, (off + 25.0 * (f * freq).sin()).clamp(5.0, 95.0))
            })
            .collect()
    };
    let mut chart = TimeSeriesChart::new_static(vec![
        Series::line("CPU", mk(0.08, 45.0)),
        Series::line("Memory", mk(0.12, 55.0)),
    ]);
    chart.set_smoothing(0.3);
    chart.set_y_config(YAxisConfig {
        min: Some(0.0),
        max: Some(100.0),
        format: Some(|v| format!("{v:.0}%")),
    });
    chart.set_guidelines(vec![Guideline {
        y_value: 80.0,
        color: Color::Rgb(0xf5, 0xc2, 0x42),
        label: Some("80%".into()),
    }]);
    let out = render(&TimeSeriesView::new(chart), 80, 22);

    assert!(braille_count(&out) > 20);
    assert!(out.contains("80%"), "guideline label renders");
    assert!(out.contains('%'), "percent-formatted Y axis renders");
    assert!(
        out.contains("CPU") && out.contains("Memory"),
        "legend renders"
    );
}

/// "Empty": no series → the empty-state message.
#[test]
fn empty_demo_renders_no_data() {
    let out = render(
        &TimeSeriesView::new(TimeSeriesChart::new_static(Vec::new())),
        40,
        10,
    );
    assert!(out.contains("No data"));
}

/// "Live": streaming construction with pushed points renders.
#[test]
fn live_demo_streams() {
    use rdom_charts::{ConnectPolicy, SeriesStyle};
    let view = TimeSeriesView::new(TimeSeriesChart::new(60.0));
    view.with(|c| {
        c.add_series(
            "Requests/s",
            Color::Rgb(0x36, 0xd0, 0xd8),
            SeriesStyle::Line,
            ConnectPolicy::Gap,
        );
        for s in 0..120 {
            let t = s as f64 * 0.5;
            c.push_points(0, &[DataPoint::new(t, 200.0 + 80.0 * (t * 0.1).sin())]);
        }
        c.tick(60.0);
    });
    assert!(braille_count(&render(&view, 80, 20)) > 10);
}
