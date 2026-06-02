//! Integration test: a `TimeSeriesView` mounted on a `<canvas>` paints
//! braille glyphs through the real rdom-tui cascade → layout → paint
//! pipeline (headless, no terminal).

use rdom_extensions::chart::{DataPoint, Series, TimeSeriesChart, TimeSeriesView};
use rdom_tui::render::{Buffer, LayoutExt, PaintExt, Rect};
use rdom_tui::style::{CascadeExt, Stylesheet, TuiStyle};
use rdom_tui::{Size, TuiDom};

/// cascade → layout → paint into a fresh buffer.
fn render(dom: &mut TuiDom, sheet: &Stylesheet, viewport: Rect) -> Buffer {
    dom.cascade(sheet);
    dom.layout_dom(viewport);
    let mut buf = Buffer::empty(viewport);
    dom.paint_dom(&mut buf, viewport);
    buf
}

fn count_braille(buf: &Buffer) -> usize {
    let area = buf.area;
    let mut n = 0;
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            if let Some(c) = buf.cell(x, y) {
                if let Some(ch) = c.symbol().chars().next() {
                    if ('\u{2800}'..='\u{28FF}').contains(&ch) {
                        n += 1;
                    }
                }
            }
        }
    }
    n
}

fn cell_at(buf: &Buffer, x: u16, y: u16) -> String {
    buf.cell(x, y)
        .map(|c| c.symbol().to_string())
        .unwrap_or_default()
}

#[test]
fn chart_paints_braille_into_buffer() {
    let series = vec![
        Series::line(
            "A",
            (0..30)
                .map(|i| DataPoint::new(i as f64 * 60.0, (i as f64 * 0.3).sin() * 50.0 + 50.0))
                .collect(),
        ),
        Series::line(
            "B",
            (0..30)
                .map(|i| DataPoint::new(i as f64 * 60.0, (i as f64 * 0.5).cos() * 30.0 + 40.0))
                .collect(),
        ),
    ];
    let view = TimeSeriesView::new(TimeSeriesChart::new_static(series));

    let mut dom = TuiDom::new();
    let root = dom.root();
    let canvas = view.mount(&mut dom);
    dom.append_child(root, canvas).unwrap();

    let viewport = Rect::new(0, 0, 80, 24);
    let sheet = Stylesheet::new()
        .rule(
            "canvas",
            TuiStyle::new()
                .width(Size::Fixed(80))
                .height(Size::Fixed(24)),
        )
        .unwrap();

    let buf = render(&mut dom, &sheet, viewport);
    let braille = count_braille(&buf);
    assert!(
        braille > 20,
        "expected the chart line to rasterize into braille glyphs, got {braille}"
    );
}

#[test]
fn empty_chart_paints_no_data_message() {
    let view = TimeSeriesView::new(TimeSeriesChart::new_static(Vec::new()));

    let mut dom = TuiDom::new();
    let root = dom.root();
    let canvas = view.mount(&mut dom);
    dom.append_child(root, canvas).unwrap();

    let viewport = Rect::new(0, 0, 40, 10);
    let sheet = Stylesheet::new()
        .rule(
            "canvas",
            TuiStyle::new()
                .width(Size::Fixed(40))
                .height(Size::Fixed(10)),
        )
        .unwrap();

    let buf = render(&mut dom, &sheet, viewport);
    // "No data" is centered on the middle row.
    let mut found = false;
    for y in 0..10 {
        let row: String = (0..40).map(|x| cell_at(&buf, x, y)).collect();
        if row.contains("No data") {
            found = true;
            break;
        }
    }
    assert!(found, "expected 'No data' message on the empty chart");
}

#[test]
fn streaming_view_updates_through_handle() {
    let view = TimeSeriesView::new(TimeSeriesChart::new(60.0));
    view.with(|c| {
        c.add_series(
            "live",
            rdom_extensions::palette::series_color(0),
            rdom_extensions::chart::SeriesStyle::Line,
            rdom_extensions::chart::ConnectPolicy::Gap,
        );
        for i in 0..60 {
            c.push_points(
                0,
                &[DataPoint::new(
                    i as f64,
                    (i as f64 * 0.2).sin() * 40.0 + 50.0,
                )],
            );
        }
        c.tick(59.0);
    });

    let mut dom = TuiDom::new();
    let root = dom.root();
    let canvas = view.mount(&mut dom);
    dom.append_child(root, canvas).unwrap();

    let viewport = Rect::new(0, 0, 80, 20);
    let sheet = Stylesheet::new()
        .rule(
            "canvas",
            TuiStyle::new()
                .width(Size::Fixed(80))
                .height(Size::Fixed(20)),
        )
        .unwrap();

    let buf = render(&mut dom, &sheet, viewport);
    assert!(count_braille(&buf) > 10, "streaming data should rasterize");
}
