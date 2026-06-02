//! A static dashboard showing every rdom-charts component at once.
//!
//! Run it:
//!
//! ```bash
//! cargo run --example dashboard
//! ```
//!
//! Press **Ctrl-C** to exit (the terminal is restored automatically).
//!
//! The layout idiom: components are flex items sized through `TuiStyle`
//! inline styles. `display: flex` is `Display::Block` + `Flow::Flex`,
//! and flex distribution reads direction/gap/size from the *computed*
//! style — so everything goes through `set_inline_style`, not the `ext`
//! node setters. (The headless twin of this build lives in
//! `tests/render_dashboard.rs`.)

use std::io;

use rdom_charts::{
    Bar, BarChart, BarChartView, DataPoint, Gauge, GaugeView, GaugeZone, Series, Sparkline,
    SparklineView, TimeSeriesChart, TimeSeriesView,
};
use rdom_tui::{
    App, Color, Direction, NodeId, Padding, Size, Stylesheet, TuiDom, TuiNodeMutExt, TuiStyle,
};

fn flex(dir: Direction) -> TuiStyle {
    match dir {
        Direction::Row => TuiStyle::new().flex_row(),
        Direction::Column => TuiStyle::new().flex_column(),
    }
}

fn style(dom: &mut TuiDom, id: NodeId, s: TuiStyle) {
    dom.node_mut(id).set_inline_style(s);
}

fn label(dom: &mut TuiDom, parent: NodeId, text: &str) {
    let p = dom.create_element("div");
    style(dom, p, TuiStyle::new().height(Size::Fixed(1)));
    let t = dom.create_text_node(text);
    dom.append_child(p, t).unwrap();
    dom.append_child(parent, p).unwrap();
}

fn build(dom: &mut TuiDom) {
    let root = dom.root();
    style(dom, root, flex(Direction::Column));

    let container = dom.create_element("div");
    style(
        dom,
        container,
        flex(Direction::Column)
            .width(Size::Flex(1))
            .height(Size::Flex(1))
            .padding(Padding::all(1))
            .gap(1),
    );
    dom.append_child(root, container).unwrap();

    label(dom, container, "rdom-charts — dashboard   (Ctrl-C to quit)");

    // Gauges.
    let gauges = dom.create_element("div");
    style(
        dom,
        gauges,
        flex(Direction::Row)
            .width(Size::Flex(1))
            .height(Size::Fixed(1))
            .gap(3),
    );
    dom.append_child(container, gauges).unwrap();

    let zones = vec![
        GaugeZone::new(70.0, Color::Rgb(0x3f, 0xc9, 0x6b)),
        GaugeZone::new(90.0, Color::Rgb(0xf5, 0xc2, 0x42)),
        GaugeZone::new(100.0, Color::Rgb(0xff, 0x5c, 0x5c)),
    ];
    for (lbl, value) in [("cpu", 72.0), ("mem", 45.0)] {
        let g = GaugeView::new(
            Gauge::new(value, 0.0, 100.0)
                .with_label(lbl)
                .with_zones(zones.clone()),
        )
        .mount(dom);
        style(
            dom,
            g,
            TuiStyle::new().width(Size::Flex(1)).height(Size::Fixed(1)),
        );
        dom.append_child(gauges, g).unwrap();
    }

    // Bar chart.
    label(dom, container, "requests / route");
    let bars = BarChartView::new(BarChart::new(vec![
        Bar::new("/", 1200.0),
        Bar::new("/api", 860.0),
        Bar::new("/health", 240.0),
        Bar::new("/static", 410.0),
    ]))
    .mount(dom);
    style(
        dom,
        bars,
        TuiStyle::new().width(Size::Flex(1)).height(Size::Fixed(4)),
    );
    dom.append_child(container, bars).unwrap();

    // Sparkline.
    let spark_row = dom.create_element("div");
    style(
        dom,
        spark_row,
        flex(Direction::Row).height(Size::Fixed(2)).gap(1),
    );
    dom.append_child(container, spark_row).unwrap();
    label(dom, spark_row, "load");
    let spark = SparklineView::new(Sparkline::new(
        (0..60).map(|i| (i as f64 * 0.3).sin() + 1.0).collect(),
    ))
    .mount(dom);
    style(
        dom,
        spark,
        TuiStyle::new()
            .width(Size::Fixed(40))
            .height(Size::Fixed(2)),
    );
    dom.append_child(spark_row, spark).unwrap();

    // Time-series.
    label(dom, container, "latency (ms)");
    let ts = TimeSeriesView::new(TimeSeriesChart::new_static(vec![
        Series::line(
            "p50",
            (0..120)
                .map(|i| DataPoint::new(i as f64, (i as f64 * 0.1).sin() * 20.0 + 60.0))
                .collect(),
        ),
        Series::line(
            "p99",
            (0..120)
                .map(|i| DataPoint::new(i as f64, (i as f64 * 0.07).cos() * 40.0 + 120.0))
                .collect(),
        ),
    ]))
    .mount(dom);
    style(
        dom,
        ts,
        TuiStyle::new().width(Size::Flex(1)).height(Size::Flex(1)),
    );
    dom.append_child(container, ts).unwrap();
}

fn main() -> io::Result<()> {
    let mut dom = TuiDom::new();
    build(&mut dom);
    App::new(dom, Stylesheet::new())?.run()
}
