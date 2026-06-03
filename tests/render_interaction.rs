//! Interaction wiring: `TimeSeriesView::install_interaction` listeners
//! mutate the chart and request a repaint (rdom 0.3 `request_redraw`).
//! Driven headlessly by dispatching synthetic keyboard events.

use rdom_charts::{TimeSeriesChart, TimeSeriesView};
use rdom_tui::{Event, EventDetail, KeyboardDetail, KeyboardModifiers, TuiDom};

fn keydown(key: &str) -> Event {
    let mut e = Event::new("keydown");
    e.detail = EventDetail::Keyboard(Box::new(KeyboardDetail {
        key: key.into(),
        modifiers: KeyboardModifiers {
            ctrl: false,
            shift: false,
            alt: false,
            meta: false,
        },
        repeat: false,
    }));
    e
}

fn mounted() -> (TimeSeriesView, TuiDom, rdom_tui::NodeId) {
    let view = TimeSeriesView::new(TimeSeriesChart::new(60.0));
    let mut dom = TuiDom::new();
    let root = dom.root();
    let canvas = view.mount(&mut dom);
    dom.append_child(root, canvas).unwrap();
    view.install_interaction(&mut dom, canvas);
    (view, dom, canvas)
}

#[test]
fn zoom_in_key_shrinks_window_and_requests_redraw() {
    let (view, mut dom, canvas) = mounted();
    let before = view.with(|c| c.window_duration());

    let mut e = keydown("+");
    dom.dispatch_event(canvas, &mut e).unwrap();

    let after = view.with(|c| c.window_duration());
    assert!(
        after < before,
        "'+' should zoom in (shrink window): {before} -> {after}"
    );
    assert!(
        e.redraw_requested(),
        "an interaction that changed the chart must request a repaint"
    );
}

#[test]
fn zoom_out_key_grows_window() {
    let (view, mut dom, canvas) = mounted();
    let before = view.with(|c| c.window_duration());
    let mut e = keydown("-");
    dom.dispatch_event(canvas, &mut e).unwrap();
    assert!(view.with(|c| c.window_duration()) > before);
    assert!(e.redraw_requested());
}

#[test]
fn pan_key_disables_follow() {
    let (view, mut dom, canvas) = mounted();
    view.with(|c| c.tick(100.0)); // following
    assert!(view.with(|c| c.is_following()));

    let mut e = keydown("h"); // pan left
    dom.dispatch_event(canvas, &mut e).unwrap();
    assert!(
        !view.with(|c| c.is_following()),
        "panning leaves follow mode"
    );
    assert!(e.redraw_requested());
}

#[test]
fn unhandled_key_requests_no_redraw() {
    let (_view, mut dom, canvas) = mounted();
    let mut e = keydown("x");
    dom.dispatch_event(canvas, &mut e).unwrap();
    assert!(!e.redraw_requested());
}

#[test]
fn canvas_is_made_focusable() {
    let (_view, dom, canvas) = mounted();
    assert_eq!(dom.node(canvas).get_attribute("tabindex"), Some("0"));
}

/// The interactive example focuses the chart canvas so keys work
/// immediately. That focus must NOT paint a background over the chart — the
/// canvas stays transparent with no consumer override. rdom 0.3.4 scopes the
/// UA focus tint to interactive controls, so a `<canvas>` (a replaced/content
/// element the app paints) is never tinted. (Earlier: 0.3.1 carried an
/// explicit `canvas:focus` exemption; 0.3.0 filled it gray via the generic
/// `:focus` tint.)
#[test]
fn focused_chart_canvas_keeps_transparent_background() {
    use rdom_tui::style::{CascadeExt, Color, Stylesheet};
    use rdom_tui::{Size, TuiNodeExt, TuiNodeMutExt, TuiStyle};

    let (_view, mut dom, canvas) = mounted();
    dom.node_mut(canvas).set_inline_style(
        TuiStyle::new()
            .width(Size::Fixed(40))
            .height(Size::Fixed(10)),
    );
    dom.set_focused(Some(canvas));
    dom.cascade(&Stylesheet::new()); // includes the UA sheet

    assert_eq!(
        dom.node(canvas).computed().unwrap().bg,
        Color::Reset,
        "a focused chart canvas must stay transparent (rdom 0.3.4 control-scoped focus tint)"
    );
}
