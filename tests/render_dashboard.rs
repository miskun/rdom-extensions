//! Composition test: several components laid out together in one tree,
//! rendered headlessly. Validates the layout idiom the `dashboard`
//! example uses — flex containers + sizing driven through `TuiStyle`
//! inline styles (the cascade/computed path that flex layout reads;
//! the `ext` node setters are NOT read by flex distribution). Run with
//! `--nocapture` to eyeball the frame.

use rdom_charts::{
    Bar, BarChart, BarChartView, DataPoint, Gauge, GaugeView, GaugeZone, Series, Sparkline,
    SparklineView, TimeSeriesChart, TimeSeriesView,
};
use rdom_tui::render::{Buffer, LayoutExt, PaintExt, Rect};
use rdom_tui::style::{CascadeExt, Stylesheet};
use rdom_tui::{Color, Direction, NodeId, Padding, Size, TuiDom, TuiNodeMutExt, TuiStyle};

/// A `display: flex` style in `dir`, via the 0.3 convenience builders.
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

/// Build the dashboard subtree. Shared shape with the `dashboard`
/// example; kept in sync by eye.
pub fn build_dashboard(dom: &mut TuiDom) {
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

    label(dom, container, "rdom-charts — dashboard");

    // Two gauges side by side.
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
    for (label_txt, value) in [("cpu", 72.0), ("mem", 45.0)] {
        let g = GaugeView::new(
            Gauge::new(value, 0.0, 100.0)
                .with_label(label_txt)
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

    // Sparkline row.
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

    // Time-series fills the remaining height.
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

fn dump(buf: &Buffer) -> String {
    let area = buf.area;
    let mut s = String::new();
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
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

#[test]
fn dashboard_lays_out_and_paints() {
    let mut dom = TuiDom::new();
    build_dashboard(&mut dom);

    let viewport = Rect::new(0, 0, 90, 34);
    let sheet = Stylesheet::new();
    dom.cascade(&sheet);
    dom.layout_dom(viewport);
    let mut buf = Buffer::empty(viewport);
    dom.paint_dom(&mut buf, viewport);

    let text = dump(&buf);
    println!("\n{text}");

    assert!(text.contains("dashboard"), "title");
    assert!(text.contains("cpu") && text.contains("mem"), "gauge labels");
    assert!(text.contains('\u{2588}'), "bar/gauge block fill");
    assert!(text.contains("/api"), "bar label");
    assert!(
        text.chars().any(|c| ('\u{2800}'..='\u{28FF}').contains(&c)),
        "braille from sparkline / time-series"
    );
    assert!(text.contains("p50") && text.contains("p99"), "ts legend");
}
