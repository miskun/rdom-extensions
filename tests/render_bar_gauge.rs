//! Integration tests: bar chart and gauge paint block glyphs / labels
//! through the real rdom-tui render pipeline (headless).

use rdom_charts::{Bar, BarChart, BarChartView, Gauge, GaugeView, GaugeZone};
use rdom_tui::render::{Buffer, LayoutExt, PaintExt, Rect};
use rdom_tui::style::{CascadeExt, Stylesheet, TuiStyle};
use rdom_tui::{Color, Size, TuiDom};

fn render(dom: &mut TuiDom, sheet: &Stylesheet, viewport: Rect) -> Buffer {
    dom.cascade(sheet);
    dom.layout_dom(viewport);
    let mut buf = Buffer::empty(viewport);
    dom.paint_dom(&mut buf, viewport);
    buf
}

fn has_full_block(buf: &Buffer) -> bool {
    let area = buf.area;
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if let Some(c) = buf.cell(x, y) {
                if c.symbol() == "\u{2588}" {
                    return true;
                }
            }
        }
    }
    false
}

fn row_text(buf: &Buffer, y: u16) -> String {
    let area = buf.area;
    (area.x..area.right())
        .filter_map(|x| buf.cell(x, y).map(|c| c.symbol().to_string()))
        .collect()
}

fn sized_sheet(w: u16, h: u16) -> Stylesheet {
    Stylesheet::new()
        .rule(
            "canvas",
            TuiStyle::new().width(Size::Fixed(w)).height(Size::Fixed(h)),
        )
        .unwrap()
}

#[test]
fn bar_chart_paints_blocks_and_labels() {
    let chart = BarChart::new(vec![
        Bar::new("alpha", 30.0),
        Bar::new("beta", 60.0),
        Bar::new("gamma", 90.0),
    ]);
    let view = BarChartView::new(chart);

    let mut dom = TuiDom::new();
    let root = dom.root();
    let canvas = view.mount(&mut dom);
    dom.append_child(root, canvas).unwrap();

    let viewport = Rect::new(0, 0, 40, 3);
    let buf = render(&mut dom, &sized_sheet(40, 3), viewport);

    assert!(has_full_block(&buf), "expected filled bar blocks");
    assert!(
        row_text(&buf, 0).contains("alpha"),
        "expected the first bar's label"
    );
}

#[test]
fn longer_bar_has_more_blocks() {
    let chart = BarChart::new(vec![Bar::new("a", 10.0), Bar::new("b", 100.0)]).without_values();
    let view = BarChartView::new(chart);

    let mut dom = TuiDom::new();
    let root = dom.root();
    let canvas = view.mount(&mut dom);
    dom.append_child(root, canvas).unwrap();

    let viewport = Rect::new(0, 0, 40, 2);
    let buf = render(&mut dom, &sized_sheet(40, 2), viewport);

    let blocks_in = |y: u16| {
        row_text(&buf, y)
            .chars()
            .filter(|&c| c == '\u{2588}')
            .count()
    };
    assert!(
        blocks_in(1) > blocks_in(0),
        "the larger value should render a longer bar"
    );
}

#[test]
fn gauge_paints_fill_and_readout() {
    let gauge = Gauge::new(72.0, 0.0, 100.0)
        .with_label("cpu")
        .with_zones(vec![
            GaugeZone::new(70.0, Color::Rgb(0, 200, 0)),
            GaugeZone::new(90.0, Color::Rgb(230, 180, 0)),
            GaugeZone::new(100.0, Color::Rgb(220, 0, 0)),
        ]);
    let view = GaugeView::new(gauge);

    let mut dom = TuiDom::new();
    let root = dom.root();
    let canvas = view.mount(&mut dom);
    dom.append_child(root, canvas).unwrap();

    let viewport = Rect::new(0, 0, 40, 1);
    let buf = render(&mut dom, &sized_sheet(40, 1), viewport);

    let row = row_text(&buf, 0);
    assert!(has_full_block(&buf), "expected gauge fill blocks");
    assert!(row.contains("cpu"), "expected the gauge label");
    assert!(row.contains("72"), "expected the value readout");
}

#[test]
fn gauge_updates_through_handle() {
    let view = GaugeView::new(Gauge::new(0.0, 0.0, 100.0).without_value());
    view.with(|g| g.set_value(100.0));

    let mut dom = TuiDom::new();
    let root = dom.root();
    let canvas = view.mount(&mut dom);
    dom.append_child(root, canvas).unwrap();

    let viewport = Rect::new(0, 0, 20, 1);
    let buf = render(&mut dom, &sized_sheet(20, 1), viewport);
    // Full value → a fully-filled track.
    assert!(has_full_block(&buf), "full value should fill the track");
}
