//! An interactive time-series chart: zoom, pan, and scrub a fixed
//! dataset with the keyboard and mouse.
//!
//! Run it:
//!
//! ```bash
//! cargo run --example interactive_chart
//! ```
//!
//! - **`+` / `-`** — zoom in / out
//! - **`h` / `l`** or **`←` / `→`** — pan left / right
//! - **`0`** — reset to the full view
//! - **mouse wheel** — zoom · **left-drag** — pan
//! - **Ctrl-C** — quit
//!
//! All of it is wired by one call, `TimeSeriesView::install_interaction`,
//! whose listeners mutate the chart and call `ctx.request_redraw()` — the
//! rdom 0.3 affordance that lets an event handler repaint a `<canvas>`
//! whose paint reads external (non-DOM) state.

use std::io;

use rdom_charts::{DataPoint, Series, TimeSeriesChart, TimeSeriesView};
use rdom_tui::{App, NodeId, Padding, Size, Stylesheet, TuiDom, TuiNodeMutExt, TuiStyle};

fn style(dom: &mut TuiDom, id: NodeId, s: TuiStyle) {
    dom.node_mut(id).set_inline_style(s);
}

fn main() -> io::Result<()> {
    // A fixed signal to explore (a slow wave + a faster ripple).
    let data: Vec<DataPoint> = (0..600)
        .map(|i| {
            let t = i as f64;
            DataPoint::new(t, (t * 0.05).sin() * 30.0 + (t * 0.2).sin() * 10.0 + 50.0)
        })
        .collect();
    let view = TimeSeriesView::new(TimeSeriesChart::new_static(vec![Series::line(
        "signal", data,
    )]));

    let mut dom = TuiDom::new();
    let root = dom.root();
    style(&mut dom, root, TuiStyle::new().flex_column());

    let container = dom.create_element("div");
    style(
        &mut dom,
        container,
        TuiStyle::new()
            .flex_column()
            .width(Size::Flex(1))
            .height(Size::Flex(1))
            .padding(Padding::all(1))
            .gap(1),
    );
    dom.append_child(root, container).unwrap();

    let title = dom.create_element("div");
    style(&mut dom, title, TuiStyle::new().height(Size::Fixed(1)));
    let t = dom.create_text_node(
        "+/- zoom · h/l or arrows pan · 0 reset · wheel zoom · drag pan · Ctrl-C quit",
    );
    dom.append_child(title, t).unwrap();
    dom.append_child(container, title).unwrap();

    let canvas = view.mount(&mut dom);
    style(
        &mut dom,
        canvas,
        TuiStyle::new().width(Size::Flex(1)).height(Size::Flex(1)),
    );
    dom.append_child(container, canvas).unwrap();

    // One call wires keyboard + wheel + drag. Focus the canvas so the
    // keys work immediately (clicking it or pressing Tab also focuses).
    view.install_interaction(&mut dom, canvas);
    dom.set_focused(Some(canvas));

    App::new(dom, Stylesheet::new())?.run()
}
