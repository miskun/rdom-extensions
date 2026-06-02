//! Time-series gallery — seven time-series demos (smooth, spiky, single,
//! dense, live, live-spotty, empty), made navigable.
//!
//! ```bash
//! cargo run --example timeseries_gallery
//! ```
//!
//! - **`1`–`7`** — switch demo (Smooth, Spiky, Single, Dense, Live,
//!   Live Spotty, Empty)
//! - **`+` / `-`** zoom · **`h` / `l`** or arrows pan · **`0`** reset
//! - mouse **wheel** zoom · **left-drag** pan · **Ctrl-C** quit
//!
//! The two "Live" demos stream points on the App's idle tick. Threshold
//! lines (Smooth's 80%, Spiky's 100ms/150ms) use `Guideline`.

use std::cell::RefCell;
use std::io;
use std::rc::Rc;
use std::time::Instant;

use rdom_extensions::chart::{
    ConnectPolicy, DataPoint, Guideline, Series, SeriesStyle, TimeSeriesChart, TimeSeriesView,
    YAxisConfig,
};
use rdom_tui::{
    App, Color, ControlFlow, ListenerOptions, NodeId, Padding, Size, Stylesheet, TuiDom,
    TuiNodeExt, TuiNodeMutExt, TuiStyle,
};

const CYAN: Color = Color::Rgb(0x36, 0xd0, 0xd8);
const PURPLE: Color = Color::Rgb(0xb4, 0x7c, 0xff);
const LIME: Color = Color::Rgb(0xa6, 0xe2, 0x2e);
const ORANGE: Color = Color::Rgb(0xff, 0x9f, 0x40);
const PINK: Color = Color::Rgb(0xff, 0x6b, 0xc2);
const BLUE: Color = Color::Rgb(0x4f, 0x9d, 0xff);
const WARN: Color = Color::Rgb(0xf5, 0xc2, 0x42);
const DANGER: Color = Color::Rgb(0xff, 0x5c, 0x5c);

const TITLES: [&str; 7] = [
    "Smooth",
    "Spiky",
    "Single",
    "Dense",
    "Live",
    "Live Spotty",
    "Empty",
];

/// Which streaming behavior (if any) the current demo wants on tick.
#[derive(Clone, Copy, PartialEq)]
enum Live {
    No,
    Steady,
    Spotty,
}

fn gen_series(points: usize, interval: f64, f: impl Fn(f64) -> f64) -> Vec<DataPoint> {
    (0..points)
        .map(|i| DataPoint::new(i as f64 * interval, f(i as f64)))
        .collect()
}

fn pct_axis() -> YAxisConfig {
    YAxisConfig {
        min: Some(0.0),
        max: Some(100.0),
        format: Some(|v| format!("{v:.0}%")),
    }
}

/// Build demo `i`: returns the view + whether it streams.
fn build(i: usize) -> (TimeSeriesView, Live) {
    match i {
        // Smooth — 3 series, EMA, % axis, 80% guideline.
        0 => {
            let mut c = TimeSeriesChart::new_static(vec![
                Series::line(
                    "CPU",
                    gen_series(120, 30.0, |i| {
                        (45.0 + 25.0 * (i * 0.08).sin()).clamp(5.0, 95.0)
                    }),
                )
                .with_color(CYAN),
                Series::line(
                    "Memory",
                    gen_series(120, 30.0, |i| {
                        (55.0 + i * 0.12 + 8.0 * (i * 0.12).sin()).clamp(10.0, 95.0)
                    }),
                )
                .with_color(PURPLE),
                Series::line(
                    "Net I/O",
                    gen_series(120, 30.0, |i| {
                        (20.0 + 15.0 * (i * 0.15).sin().abs()).clamp(0.0, 60.0)
                    }),
                )
                .with_color(LIME),
            ]);
            c.set_smoothing(0.3);
            c.set_y_config(pct_axis());
            c.set_guidelines(vec![Guideline {
                y_value: 80.0,
                color: WARN,
                label: Some("80%".into()),
            }]);
            (TimeSeriesView::new(c), Live::No)
        }
        // Spiky — 2 series, spikes, 100ms/150ms thresholds.
        1 => {
            let latency = gen_series(200, 15.0, |i| {
                let base = 50.0 + 20.0 * (i * 0.05).sin();
                let spike = if (i * 7.3).sin() > 0.85 { 80.0 } else { 0.0 };
                (base + spike + 15.0 * (i * 1.7).sin()).clamp(5.0, 200.0)
            });
            let errors = gen_series(200, 15.0, |i| {
                let burst: f64 = if (i * 5.3).sin() > 0.9 { 25.0 } else { 0.0 };
                (2.0 + burst).clamp(0.0, 50.0)
            });
            let mut c = TimeSeriesChart::new_static(vec![
                Series::line("Latency (ms)", latency).with_color(ORANGE),
                Series::line("Error rate", errors).with_color(DANGER),
            ]);
            c.set_y_config(YAxisConfig {
                min: Some(0.0),
                max: None,
                format: None,
            });
            c.set_guidelines(vec![
                Guideline {
                    y_value: 100.0,
                    color: WARN,
                    label: Some("100ms".into()),
                },
                Guideline {
                    y_value: 150.0,
                    color: DANGER,
                    label: Some("150ms".into()),
                },
            ]);
            (TimeSeriesView::new(c), Live::No)
        }
        // Single — one series, °C axis.
        2 => {
            let mut c = TimeSeriesChart::new_static(vec![
                Series::line(
                    "Temperature",
                    gen_series(60, 60.0, |i| 50.0 + 40.0 * (i * 0.1).sin()),
                )
                .with_color(BLUE),
            ]);
            c.set_y_config(YAxisConfig {
                min: Some(0.0),
                max: Some(100.0),
                format: Some(|v| format!("{v:.0}\u{00b0}C")),
            });
            (TimeSeriesView::new(c), Live::No)
        }
        // Dense — 5 series, light smoothing.
        3 => {
            let defs = [
                ("Pod 1", CYAN, 0.07, 30.0),
                ("Pod 2", PURPLE, 0.09, 35.0),
                ("Pod 3", LIME, 0.11, 40.0),
                ("Pod 4", ORANGE, 0.06, 45.0),
                ("Pod 5", PINK, 0.13, 25.0),
            ];
            let series = defs
                .iter()
                .map(|&(n, col, freq, off)| {
                    Series::line(
                        n,
                        gen_series(500, 5.0, |i| {
                            (off + 20.0 * (i * freq).sin() + 8.0 * (i * freq * 2.3).cos())
                                .clamp(5.0, 80.0)
                        }),
                    )
                    .with_color(col)
                })
                .collect();
            let mut c = TimeSeriesChart::new_static(series);
            c.set_smoothing(0.2);
            c.set_y_config(pct_axis());
            (TimeSeriesView::new(c), Live::No)
        }
        // Live — 2 series streaming, 60s window, smoothing.
        4 => {
            let v = TimeSeriesView::new(TimeSeriesChart::new(60.0));
            v.with(|c| {
                c.add_series("Requests/s", CYAN, SeriesStyle::Line, ConnectPolicy::Gap);
                c.add_series(
                    "Latency (ms)",
                    ORANGE,
                    SeriesStyle::Line,
                    ConnectPolicy::Gap,
                );
                c.set_smoothing(0.3);
                c.set_y_config(YAxisConfig {
                    min: Some(0.0),
                    max: None,
                    format: None,
                });
                c.tick(0.0);
            });
            (v, Live::Steady)
        }
        // Live Spotty — Sensor A steady, Sensor B irregular gaps.
        5 => {
            let v = TimeSeriesView::new(TimeSeriesChart::new(60.0));
            v.with(|c| {
                c.add_series("Sensor A", CYAN, SeriesStyle::Line, ConnectPolicy::Gap);
                c.add_series("Sensor B", PINK, SeriesStyle::Line, ConnectPolicy::Gap);
                c.set_y_config(pct_axis());
                c.tick(0.0);
            });
            (v, Live::Spotty)
        }
        // Empty.
        _ => (
            TimeSeriesView::new(TimeSeriesChart::new_static(Vec::new())),
            Live::No,
        ),
    }
}

struct Gallery {
    index: usize,
    view: TimeSeriesView,
    live: Live,
    canvas: NodeId,
    title_text: NodeId,
    start: Instant,
    next_b: f64,
    drag: Option<i32>,
}

fn title_str(index: usize) -> String {
    format!(
        "[{}/7] {}   ·   1-7 switch · +/- zoom · h/l pan · 0 reset · Ctrl-C quit",
        index + 1,
        TITLES[index]
    )
}

impl Gallery {
    /// Switch to demo `index` by re-pointing the existing canvas's paint
    /// at the new view — no node drop / remount / focus change, so it's
    /// safe to call from inside the keydown handler (dropping the focused
    /// canvas mid-dispatch would fire re-entrant blur/focusout and crash
    /// the loop).
    fn switch(&mut self, dom: &mut TuiDom, index: usize) {
        let (view, live) = build(index);
        view.attach(dom, self.canvas);
        let _ = dom
            .node_mut(self.title_text)
            .set_node_value(&title_str(index));
        self.index = index;
        self.view = view;
        self.live = live;
        self.start = Instant::now();
        self.next_b = 0.0;
        self.drag = None;
    }

    /// Push streaming data for the live demos. Returns true if it drew.
    fn stream(&mut self) -> bool {
        if self.live == Live::No {
            return false;
        }
        let t = self.start.elapsed().as_secs_f64();
        self.view.with(|c| match self.live {
            Live::Steady => {
                c.push_points(0, &[DataPoint::new(t, 200.0 + 80.0 * (t * 0.4).sin())]);
                c.push_points(1, &[DataPoint::new(t, 45.0 + 20.0 * (t * 0.6).sin())]);
                c.tick(t);
            }
            Live::Spotty => {
                c.push_points(
                    0,
                    &[DataPoint::new(
                        t,
                        (50.0 + 30.0 * (t * 0.5).sin()).clamp(5.0, 95.0),
                    )],
                );
                if t >= self.next_b {
                    c.push_points(
                        1,
                        &[DataPoint::new(
                            t,
                            (40.0 + 25.0 * (t * 0.7).sin()).clamp(5.0, 95.0),
                        )],
                    );
                    self.next_b = t + 1.0 + 3.0 * (t * 2.7).sin().abs();
                }
                c.tick(t);
            }
            Live::No => {}
        });
        true
    }
}

fn style(dom: &mut TuiDom, id: NodeId, s: TuiStyle) {
    dom.node_mut(id).set_inline_style(s);
}

fn flex_col() -> TuiStyle {
    TuiStyle::new().flex_column()
}

fn main() -> io::Result<()> {
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
    let title_text = dom.create_text_node(&title_str(0));
    dom.append_child(title, title_text).unwrap();
    dom.append_child(container, title).unwrap();

    let (view, live) = build(0);
    let canvas = view.mount(&mut dom);
    style(
        &mut dom,
        canvas,
        TuiStyle::new().width(Size::Flex(1)).height(Size::Flex(1)),
    );
    dom.append_child(container, canvas).unwrap();
    dom.set_focused(Some(canvas));

    let gallery = Rc::new(RefCell::new(Gallery {
        index: 0,
        view,
        live,
        canvas,
        title_text,
        start: Instant::now(),
        next_b: 0.0,
        drag: None,
    }));

    // All interaction lives on the root so a canvas swap never loses a
    // listener; handlers operate on the gallery's *current* view.
    let g = gallery.clone();
    dom.add_event_listener(root, "keydown", ListenerOptions::default(), move |ctx| {
        let Some(key) = ctx.event.detail.as_keyboard() else {
            return;
        };
        let mut g = g.borrow_mut();
        match key.key.as_str() {
            d @ ("1" | "2" | "3" | "4" | "5" | "6" | "7") => {
                let idx = d.parse::<usize>().unwrap() - 1;
                if idx != g.index {
                    g.switch(ctx.dom, idx);
                }
            }
            "+" | "=" => g.view.with(|c| c.zoom(0.5)),
            "-" => g.view.with(|c| c.zoom(2.0)),
            "h" | "ArrowLeft" => g.view.with(|c| c.pan_by_fraction(-0.1)),
            "l" | "ArrowRight" => g.view.with(|c| c.pan_by_fraction(0.1)),
            "0" => g.view.with(|c| c.reset_to_live()),
            _ => return,
        }
        ctx.event.prevent_default();
        ctx.request_redraw();
    })
    .unwrap();

    let g = gallery.clone();
    dom.add_event_listener(root, "wheel", ListenerOptions::default(), move |ctx| {
        let Some(m) = ctx.event.detail.as_mouse() else {
            return;
        };
        let factor = if m.delta_y < 0 { 0.8 } else { 1.25 };
        g.borrow().view.with(|c| c.zoom(factor));
        ctx.event.prevent_default();
        ctx.request_redraw();
    })
    .unwrap();

    let g = gallery.clone();
    dom.add_event_listener(root, "mousedown", ListenerOptions::default(), move |ctx| {
        if let Some(m) = ctx.event.detail.as_mouse() {
            g.borrow_mut().drag = Some(m.client_x);
        }
    })
    .unwrap();

    let g = gallery.clone();
    dom.add_event_listener(root, "mousemove", ListenerOptions::default(), move |ctx| {
        let mut g = g.borrow_mut();
        let Some(start) = g.drag else { return };
        let Some(m) = ctx.event.detail.as_mouse() else {
            return;
        };
        let dx = m.client_x - start;
        if dx == 0 {
            return;
        }
        let width = ctx
            .dom
            .node(g.canvas)
            .content_layout_rect()
            .map(|r| r.width)
            .unwrap_or(0);
        g.view.with(|c| c.pan_by_columns(dx, width));
        g.drag = Some(m.client_x);
        ctx.request_redraw();
    })
    .unwrap();

    let g = gallery.clone();
    dom.add_event_listener(root, "mouseup", ListenerOptions::default(), move |_ctx| {
        g.borrow_mut().drag = None;
    })
    .unwrap();

    // Stream the live demos on idle ticks.
    let g = gallery.clone();
    let app = App::new(dom, Stylesheet::new())?.on_tick(move |ctx| {
        if g.borrow_mut().stream() {
            ctx.request_redraw();
        }
        ControlFlow::Continue
    });
    app.run()
}
