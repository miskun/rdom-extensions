//! A live time-series chart that streams new samples on every tick.
//!
//! Run it:
//!
//! ```bash
//! cargo run --example live_chart
//! ```
//!
//! Press **Ctrl-C** to exit. Watch the line scroll right as points
//! arrive — the chart is in follow mode, so the 20-second window slides
//! to track the live edge.
//!
//! Pattern: `App::on_tick` fires whenever the loop is idle (default
//! ~50 ms). Each tick pushes a sample through the shared `TimeSeriesView`
//! handle and asks the runtime to repaint. The chart state lives behind
//! the view's `Rc<RefCell>`, so the tick closure mutates it and the
//! paint closure reads it on the next frame.

use std::io;
use std::time::Instant;

use rdom_charts::palette::series_color;
use rdom_charts::{ConnectPolicy, DataPoint, SeriesStyle, TimeSeriesChart, TimeSeriesView};
use rdom_tui::{
    App, ControlFlow, NodeId, Padding, Size, Stylesheet, TuiDom, TuiNodeMutExt, TuiStyle,
};

fn flex_col() -> TuiStyle {
    TuiStyle::new().flex_column()
}

fn style(dom: &mut TuiDom, id: NodeId, s: TuiStyle) {
    dom.node_mut(id).set_inline_style(s);
}

fn main() -> io::Result<()> {
    // Streaming chart: a 20-second sliding window, one smoothed series.
    let view = TimeSeriesView::new(TimeSeriesChart::new(20.0));
    view.with(|c| {
        c.add_series(
            "signal",
            series_color(0),
            SeriesStyle::Line,
            ConnectPolicy::Gap,
        );
        c.set_smoothing(0.3);
    });

    let mut dom = TuiDom::new();
    let root = dom.root();
    style(&mut dom, root, flex_col());

    let container = dom.create_element("div");
    style(
        &mut dom,
        container,
        flex_col()
            .width(Size::Flex(1))
            .height(Size::Flex(1))
            .padding(Padding::all(1))
            .gap(1),
    );
    dom.append_child(root, container).unwrap();

    let title = dom.create_element("div");
    style(&mut dom, title, TuiStyle::new().height(Size::Fixed(1)));
    let t = dom.create_text_node("live time-series — streaming (Ctrl-C to quit)");
    dom.append_child(title, t).unwrap();
    dom.append_child(container, title).unwrap();

    let canvas = view.mount(&mut dom);
    style(
        &mut dom,
        canvas,
        TuiStyle::new().width(Size::Flex(1)).height(Size::Flex(1)),
    );
    dom.append_child(container, canvas).unwrap();

    // Drive the stream from the idle tick.
    let stream = view.clone();
    let start = Instant::now();
    let app = App::new(dom, Stylesheet::new())?.on_tick(move |ctx| {
        let t = start.elapsed().as_secs_f64();
        stream.with(|c| {
            let v = (t * 1.3).sin() * 30.0 + (t * 0.5).cos() * 15.0 + 55.0;
            c.push_points(0, &[DataPoint::new(t, v)]);
            c.tick(t);
        });
        ctx.request_redraw();
        ControlFlow::Continue
    });
    app.run()
}
