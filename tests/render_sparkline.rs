//! Integration test: a `SparklineView` mounted on a `<canvas>` paints
//! braille glyphs through the real rdom-tui render pipeline (headless).

use rdom_extensions::chart::{Sparkline, SparklineView};
use rdom_tui::render::{Buffer, LayoutExt, PaintExt, Rect};
use rdom_tui::style::{CascadeExt, Stylesheet, TuiStyle};
use rdom_tui::{Size, TuiDom};

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

#[test]
fn sparkline_paints_braille_into_small_canvas() {
    let values: Vec<f64> = (0..40).map(|i| (i as f64 * 0.4).sin()).collect();
    let view = SparklineView::new(Sparkline::new(values));

    let mut dom = TuiDom::new();
    let root = dom.root();
    let canvas = view.mount(&mut dom);
    dom.append_child(root, canvas).unwrap();

    // Inline-sized: 20 wide, 2 tall.
    let viewport = Rect::new(0, 0, 20, 2);
    let sheet = Stylesheet::new()
        .rule(
            "canvas",
            TuiStyle::new()
                .width(Size::Fixed(20))
                .height(Size::Fixed(2)),
        )
        .unwrap();

    let buf = render(&mut dom, &sheet, viewport);
    assert!(
        count_braille(&buf) >= 5,
        "expected the sparkline to rasterize into braille glyphs"
    );
}

#[test]
fn streaming_sparkline_updates_through_handle() {
    let view = SparklineView::new(Sparkline::new(vec![0.0]));
    view.with(|s| s.set_values((0..30).map(|i| i as f64).collect()));

    let mut dom = TuiDom::new();
    let root = dom.root();
    let canvas = view.mount(&mut dom);
    dom.append_child(root, canvas).unwrap();

    let viewport = Rect::new(0, 0, 30, 3);
    let sheet = Stylesheet::new()
        .rule(
            "canvas",
            TuiStyle::new()
                .width(Size::Fixed(30))
                .height(Size::Fixed(3)),
        )
        .unwrap();

    let buf = render(&mut dom, &sheet, viewport);
    assert!(count_braille(&buf) > 5, "updated values should rasterize");
}
