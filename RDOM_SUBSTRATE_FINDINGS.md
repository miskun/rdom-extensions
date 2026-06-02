# rdom Substrate Findings — from building rdom-extensions

Friction, divergences, and workarounds encountered while building the data-visualization
components (`rdom-extensions`) on top of the **published `rdom-tui 0.2.0`** substrate. Written for
the rdom maintainer to triage on the rdom side.

Each item states **what we expected** (referencing the web platform where relevant), **what rdom
actually does** (with file references where known), **our workaround**, **severity**, and a
**recommendation**. "Verified" = we observed it directly this session; "Inferred" = deduced from
reading rdom source, not independently reproduced in isolation.

Legend — severity: **High** (silent footgun / blocks a use case), **Medium** (ergonomics /
discoverability), **Low** (nice-to-have / documented simplification).

---

## 1. Node setters (`set_width`/`set_direction`/`set_gap`/…) are ignored by flex layout — High · Verified

**Expected.** Setting layout properties through the public node API affects layout. `TuiNodeMutExt`
exposes `set_width`, `set_height`, `set_direction`, `set_gap`, `set_padding`, etc., all returning
`&mut Self` for chaining, and the accessors (`node(id).width()`, `.direction()`) echo what you set.
Every signal says "this is how you set layout."

**What rdom does.** The flex layout pass reads geometry **exclusively from `ComputedStyle`**
(`crates/rdom-tui/src/render/layout_pass/flex.rs` — `layout_flex_children(…, parent: &ComputedStyle)`
then `let direction = parent.direction; let gap = parent.gap;`, and child sizing via
`dom.node(c)...computed()`). `ComputedStyle` is produced by the cascade from `TuiStyle` (author
rules + `inline_style`) only. The `ext.*` fields written by the node setters are **not an input to
the cascade**, so they never reach `ComputedStyle` and are silently ignored by flex distribution.

Concretely: a `<div>` with `set_direction(Direction::Row)` + children with `set_width(Flex(1))`
laid out as a **column** with auto-sized children — the setters had zero effect. Switching the same
properties to a `TuiStyle` inline style fixed it immediately.

**Why it's nasty.** It fails *silently and plausibly*: no panic, no warning, and the accessors
report the values you set, so introspection confirms the wrong mental model. It cost a real
debugging cycle to discover that the entire `set_*` family is inert for flex layout. This will bite
every consumer who composes a layout programmatically (i.e. without the CSS parser).

**Our workaround.** Drive *all* layout through `TuiStyle` inline styles via `set_inline_style`
(see `examples/dashboard.rs`, `tests/render_dashboard.rs` — the `flex()` / `style()` helpers). We
stopped using the geometry node setters entirely.

**Recommendation (pick one):**
- Make the `ext.*` geometry fields a cascade input (presentation override layer that merges into
  `ComputedStyle`), so the setters actually work; **or**
- If the setters are intentionally not layout inputs, **remove them** (or rename to make that
  obvious) and document that layout must go through `TuiStyle`; **or**
- At minimum, document loudly on every geometry setter that it does **not** affect flex layout.

The worst outcome is the current one: a full, chainable, accessor-backed API that looks
authoritative and does nothing for layout.

---

## 2. `display: flex` has no ergonomic setter; it's two raw fields — Medium · Verified

**Expected (CSS).** `display: flex` is a single declaration.

**What rdom does.** `display` is split into outer `Display` (`Block`/`Inline`/`InlineBlock`/`None`
— **no `Flex` variant**, `crates/rdom-style/src/layout.rs:501`) and inner `Flow`
(`Block`/`Flex`, `:548`). `display: flex` = `Display::Block` + `Flow::Flex`, stored as two separate
`TuiStyle` fields (`tui_style.rs:156` `display`, `:160` `flow`). The builder `TuiStyle::display(v)`
only sets the **outer** field, and there is **no `TuiStyle::flow()` builder**. So a programmatic
consumer (no CSS string parser) must poke two public fields with `Value::Specified` wrappers:

```rust
let mut s = TuiStyle::new().direction(Direction::Row);
s.display = Some(Value::Specified(Display::Block));
s.flow    = Some(Value::Specified(Flow::Flex));
```

This is undiscoverable — `Display::Flex` is the obvious guess and doesn't exist; nothing in the
builder surface hints that flex lives on a second field.

**Our workaround.** A local `flex(dir) -> TuiStyle` helper that sets both fields (duplicated in the
example and the test).

**Recommendation.** Add a `TuiStyle::flow(Flow)` builder and a convenience such as
`TuiStyle::flex()` / `flex_row()` / `flex_column()` (sets `display: Block` + `flow: Flex` +
direction). Bonus: a `display` value parser that accepts the combined `flex` keyword for parity
with CSS.

---

## 3. Two distinct public types both named `RenderContext` — Medium · Verified

**What rdom does.** There are two different public structs named `RenderContext`:
- `rdom_tui::render::RenderContext` (`render/render_context.rs`) — event/render-callback flavor:
  `set_char`, `set_string`, `fill(rect, cell)`, `set_cell`, `set_style`; **public** constructor
  `new(area, buf, scroll)`. This is the one **re-exported at the crate root** (`lib.rs:78-81`), so
  `use rdom_tui::*` brings *this* one into scope.
- `rdom_tui::runtime::builtins::canvas::RenderContext` (`runtime/builtins/canvas/mod.rs`) — the
  canvas paint flavor: `set`, `text`, `rect(x,y,w,h,style)`, `fill(style)`, `sub`; constructor is
  `pub(crate)`. This is the one a `canvas::set_paint` callback actually receives.

So the name a consumer most easily imports is **not** the one their paint closure is handed, and the
two have different method sets (`set` vs `set_char`, `fill(style)` vs `fill(rect, cell)`). We nearly
wrote against the wrong one.

**Our workaround.** Always import the canvas one by full path
(`rdom_tui::runtime::builtins::canvas::RenderContext`) and never `use rdom_tui::*`.

**Recommendation.** Rename one (e.g. the canvas flavor → `CanvasContext` / `PaintSurface`), or stop
re-exporting `render::RenderContext` at the crate root. A single public `RenderContext` name with
two meanings is an avoidable hazard.

---

## 4. No way to request a repaint from an event listener when paint inputs live outside the DOM — High · Verified (blocks interaction)

**Expected (web).** A `click`/`keydown`/`wheel` handler mutates app state and the view re-renders.
Canvas apps call something equivalent to "invalidate / request animation frame."

**What rdom does.** Repaint is driven by the `DirtyTracker` observing **DOM mutations**. The event
listener context (`TuiEventCtx` = `rdom_core::EventCtx`) exposes `dom` and timer methods, but **no
`request_redraw`** — that lives only on `AppContext` (the tick/`on_tick` context,
`runtime/app/context.rs`) and `AppHandle`. For a `<canvas>` whose paint reads **external** state
(the pattern rdom's own `<canvas>` docs recommend — "apps that need reactive drawing call
`AppHandle::needs_redraw()`"; note the actual method is `request_redraw`), a listener that mutates
that external state produces **no DOM mutation**, so nothing marks the frame dirty and it won't
repaint.

This is the concrete blocker for our keyboard zoom/pan and mouse/scroll interaction (M7): our chart
state lives behind an `Rc<RefCell>` (because the canvas paint closure must borrow it), exactly as
this crate's components and rdom's canvas docs do — but there's no first-class "my paint inputs
changed, repaint this canvas" call reachable from inside a listener.

**Workarounds available (all unsatisfying).**
- Touch a throwaway DOM attribute on the canvas inside the listener to trip the `DirtyTracker`.
- Schedule a no-op `request_animation_frame` from the listener (the timer API *is* on
  `TuiEventCtx`) to force a frame.
- Drive everything from `on_tick` instead of listeners (what `examples/live_chart.rs` does).

**Recommendation.** Either expose `request_redraw()` on the event-listener context, or add
`canvas::request_repaint(dom, node)` / a way to mark a canvas's `canvas_paint` dirty, so the
documented "canvas reads external state" pattern is actually reactive from event handlers.

---

## 5. Detaching nodes orphans them in the arena (no reclamation) — Medium · Inferred + partially verified

**Expected (DOM).** `removeChild` / `replaceChildren` detach a node; once unreferenced it's
reclaimed.

**What rdom does.** The arena has no GC. `clear_children` / `remove_child` **detach but keep nodes
in the arena** (the docs call them "orphans … can be reattached"). Only `drop_subtree` frees the
slots. A high-churn component (e.g. a virtualized table re-materializing rows on every scroll) that
uses `clear_children`/`remove_child` to swap rows would **leak arena slots indefinitely** — silently,
since nothing errors.

**Our workaround.** The virtual table tracks the node ids it materialized and calls `drop_subtree`
on each before building the next window (`src/table/virtual_table.rs::show_window`). We deliberately
avoided `clear_children`.

**Open question for rdom.** Does `replace_children` drop the replaced nodes or orphan them? If it
orphans, it shares this footgun.

**Recommendation.** Document the orphan/leak semantics prominently on `clear_children`/`remove_child`,
and/or provide a "remove and free" convenience (e.g. `clear_children_dropping`) so high-churn UIs
don't silently grow the arena.

---

## 6. Canvas `RenderContext` can't be constructed externally → paint code isn't unit-testable — Low/Medium · Verified

**What rdom does.** The canvas `RenderContext::new` is `pub(crate)`, so a downstream crate can't
build one over a scratch `Buffer`. Any test of paint output must run the full
`cascade → layout_dom → paint_dom` pipeline and inspect the resulting buffer.

**Impact.** We split each component into pure logic (scaling/windowing/zone math — unit-tested) and
a thin `paint()` (covered only via integration tests). That's a fine discipline, but it means the
paint glue itself can't be tested in isolation; small paint regressions only surface through a full
render.

**Recommendation.** Consider a public constructor or a `RenderContext::for_test(buffer, rect)` so
downstream paint code can be unit-tested directly.

---

## 7. No virtualization primitives (post-layout viewport read, spacer for scrollbar) — Low · Inferred

**Observation, not a bug.** True scroll-driven virtualization needs (a) the laid-out viewport
height to compute the visible window — available via `content_layout_rect()` but only *after* a
layout pass, a chicken-and-egg for the first frame; and (b) a spacer sized to the *total* content so
the scrollbar/`scroll_content_height` reflects all rows while only a window is materialized. rdom
provides the pieces (`overflow`, `scroll_content_height`, `content_layout_rect`) but no helper that
ties them into "virtualized list/table." We scoped our table to explicit `show_window(start, count)`
and deferred auto-scroll wiring.

**Recommendation.** Not a substrate bug. If virtualization is meant to be first-class, a documented
recipe (or a tiny helper) for the spacer + window pattern would save every consumer from
rediscovering it.

---

## What's solid (so the review is honest)

The substrate is genuinely good; most findings above are ergonomics/discoverability, not
correctness:

- **The `<canvas>` + `set_paint` hook is exactly the right escape hatch** — bounded `RenderContext`,
  silent clipping, `sub()` for delegating sub-rects. Our entire chart family rests on it cleanly.
- **The headless `cascade → layout_dom → Buffer::empty → paint_dom` pipeline is fast and fully
  inspectable** — it's what let us TDD rendering and validate the whole dashboard without a TTY.
- **Native `<table>` column sync (`size_columns`) + `drop_subtree`** gave us a real virtual table
  on browser-faithful elements with no custom layout.
- **Event/timer model is browser-faithful** (capture/bubble, `as_keyboard`/`as_mouse`,
  `request_animation_frame`, `on_tick`), and **terminal state restores on panic** automatically.
- **Color/Style composition and braille/cell writes behaved exactly as expected** — no surprises in
  the paint primitives themselves.

---

## Triage summary

| # | Finding | Severity | Kind |
|---|---------|----------|------|
| 1 | Geometry node setters ignored by flex layout | **High** | Silent footgun |
| 4 | No repaint request from event listeners (external/canvas state) | **High** | Missing capability (blocks interaction) |
| 2 | `display: flex` needs two raw fields, no builder | Medium | Ergonomics |
| 3 | Two public types named `RenderContext` | Medium | API hazard |
| 5 | Detach orphans the arena (no reclamation) | Medium | Footgun / doc gap |
| 6 | Canvas `RenderContext` not constructible for tests | Low/Med | Testability |
| 7 | No virtualization helper (spacer + viewport read) | Low | Missing convenience |

Items **1** and **4** are the ones worth fixing on the rdom side first: #1 because it silently
misleads every programmatic consumer, and #4 because it blocks interactive canvas components — the
next thing we want to build here.
