//! # rdom-extensions
//!
//! Optional data-visualization components for [rdom](https://github.com/miskun/rdom),
//! the browser-faithful DOM for terminal applications.
//!
//! This crate is a **downstream consumer** of the rdom substrate, not
//! part of it. The core rdom workspace deliberately ships only native
//! HTML element behaviors and zero opinionated components; charts,
//! sparklines, gauges, and virtualized tables are exactly the kind of
//! higher-level components that belong outside that publish set. They
//! live here, built strictly on rdom-tui's public API:
//!
//! - the `<canvas>` paint API (`canvas::set_paint` + `RenderContext`)
//!   for pixel-style drawing (charts use a braille sub-cell grid),
//! - element builders + the cascade for layout and color,
//! - the runtime event listeners for interaction (zoom, pan, hover).
//!
//! Nothing here reaches into rdom internals, so the crate can evolve
//! independently of the substrate.
//!
//! ## Status
//!
//! Shipped: time-series line chart, sparkline, bar chart, and rich gauge
//! (all `<canvas>`-painted via a braille / block rasterizer), plus a
//! `<table>`-based virtualized table. Interaction wiring (scroll/zoom/pan
//! from events) and runnable examples are in progress; see `STATE.md`.

pub mod chart;
pub mod palette;
pub mod table;
