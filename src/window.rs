//! JEI's recipe screen: its window of recipes over the open screen, with the tabs above it, the catalysts left
//! of it and the pages inside. The places come from `runtime::screen`; this draws them with the mod API and
//! tells what the pointer is on.
use mod_api::{ClientHost, DrawContext, Identifier, Text};

use crate::glue::st;
use crate::runtime::{self as jei, Layout, Stack, View, screen as geo};

/// The recipe screen: the viewer's page, the page of it shown, and whether JEI opened a screen of its own to
/// show it (nothing was open).
#[derive(Clone, Debug)]
pub(crate) struct Overlay {
    pub(crate) title: String,
    pub(crate) tabs: Vec<Stack>,
    pub(crate) names: Vec<String>,
    pub(crate) tab: usize,
    pub(crate) views: Vec<View>,
    pub(crate) catalysts: Vec<Stack>,
    pub(crate) subtitle: String,
    pub(crate) page: usize,
    pub(crate) own_screen: bool,
    /// The notes have been wrapped to the window's width (the first draw does it: it can measure text).
    pub(crate) wrapped: bool,
    /// The size the player wants the window (JEI's `recipeGuiWidth` and `recipeGuiHeight`).
    pub(crate) size: [f32; 2],
    /// The recipes scroll, `scroll` pixels down, instead of turning pages.
    pub(crate) scrolling: bool,
    pub(crate) scroll: f32,
    /// What the pointer holds: an edge of the window or the scrollbar's marker.
    pub(crate) drag: Option<Drag>,
}

/// A press held down on the window.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Drag {
    /// An edge (left, right, top, bottom), where the pointer took it and the window's size then.
    Resize {
        edge: [bool; 4],
        start: [f32; 2],
        size: [f32; 2],
    },
    /// The scrollbar's marker, held `grab` pixels from its top.
    Bar { grab: f32 },
}

const PANEL_BG: u32 = 0xFF_C6_C6_C6;
const PANEL_LIGHT: u32 = 0xFF_FF_FF_FF;
const PANEL_DARK: u32 = 0xFF_55_55_55;
const OUTLINE: u32 = 0xFF_00_00_00;
const SLOT_BG: u32 = 0xFF_8B_8B_8B;
const SLOT_DARK: u32 = 0xFF_37_37_37;
const TEXT: u32 = 0xFF_40_40_40;
const WHITE: u32 = 0xFF_FF_FF_FF;
const DIM: u32 = 0xA0_10_10_10;
/// JEI's GUI sprites (1.12's, MIT), in this mod's assets; when they don't load the screen draws the same shapes
/// with fills.
const SPRITES: &str = "jei:gui/sprites/";

/// The recipe screen's parts now, from the GUI's size.
fn geom(o: &Overlay, gui: [f32; 2]) -> geo::Geom {
    geo::geom(
        gui,
        o.tabs.len(),
        o.tab,
        o.catalysts.len(),
        o.size,
        o.scrolling,
    )
}

/// How far the list scrolls now (0 when it pages, or fits).
pub(crate) fn max_scroll(o: &Overlay, gui: [f32; 2]) -> f32 {
    if o.scrolling {
        geo::max_scroll(&o.views, geom(o, gui).content)
    } else {
        0.0
    }
}

/// The recipes' movement: the pages, or the list at its offset (kept within what scrolls).
fn nav_of(o: &Overlay, g: &geo::Geom) -> geo::Nav {
    let max = if o.scrolling {
        geo::max_scroll(&o.views, g.content)
    } else {
        0.0
    };
    geo::Nav {
        scrolling: o.scrolling,
        offset: o.scroll.clamp(0.0, max),
    }
}

/// The scrollbar's marker share: the list's offset from 0 to 1.
fn bar_offset(o: &Overlay, max: f32) -> f32 {
    if max > 0.0 {
        (o.scroll / max).clamp(0.0, 1.0)
    } else {
        0.0
    }
}

/// A press on the scrollbar at `y`: the list jumps there when the marker was missed, and the marker is held.
pub(crate) fn bar_press(o: &mut Overlay, gui: [f32; 2], y: f32) {
    let g = geom(o, gui);
    let max = max_scroll(o, gui);
    let (offset, grab) = geo::bar_press(g.scrollbar, g.viewport[3], max, bar_offset(o, max), y);
    o.scroll = offset * max;
    o.drag = Some(Drag::Bar { grab });
}

/// A press on an edge of the window at `mouse`.
pub(crate) fn edge_press(o: &mut Overlay, gui: [f32; 2], edge: [bool; 4], mouse: [f32; 2]) {
    let size = geom(o, gui).panel;
    o.drag = Some(Drag::Resize {
        edge,
        start: mouse,
        size: [size[2], size[3]],
    });
}

/// The pointer moved to `mouse` with something held: the marker follows it, or the window's size.
/// True when the window's size changed.
pub(crate) fn drag_to(o: &mut Overlay, gui: [f32; 2], mouse: [f32; 2]) -> bool {
    match o.drag {
        Some(Drag::Bar { grab }) => {
            let g = geom(o, gui);
            let max = max_scroll(o, gui);
            let offset = geo::bar_drag(
                g.scrollbar,
                g.viewport[3],
                max,
                bar_offset(o, max),
                grab,
                mouse[1],
            );
            o.scroll = offset * max;
            false
        }
        Some(Drag::Resize { edge, start, size }) => {
            let min_w = geo::MIN_WIDTH.min(gui[0] - 12.0).max(120.0);
            let min = [min_w, geo::MIN_HEIGHT];
            let max = [(gui[0] - 12.0).max(min_w), (gui[1] - 58.0).max(geo::MIN_HEIGHT)];
            let now = geo::resized(size, edge, start, mouse, min, max);
            let changed = now != o.size;
            o.size = now;
            changed
        }
        None => false,
    }
}

/// The wheel over a scrolling list: `notches` of it (up is positive).
pub(crate) fn scroll_by(o: &mut Overlay, gui: [f32; 2], notches: f32) {
    let max = max_scroll(o, gui);
    o.scroll = (o.scroll - notches * geo::WHEEL).clamp(0.0, max);
}

/// The window's width, for the item list to lay out beside it.
pub(crate) fn width(o: &Overlay, gui: [f32; 2]) -> f32 {
    geom(o, gui).panel[2]
}

/// How many pages the selected tab has (one at least).
pub(crate) fn pages(o: &Overlay, gui: [f32; 2]) -> usize {
    let g = geom(o, gui);
    geo::pages(&o.views, g.content, nav_of(o, &g)).len().max(1)
}

/// The stack a slot of alternatives shows now: one a second, as JEI cycles.
fn cycled(stacks: &[Stack]) -> Option<&Stack> {
    if stacks.is_empty() {
        return None;
    }
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs()) as usize;
    stacks.get(secs % stacks.len())
}

/// What is under the pointer on the recipe screen.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Hit {
    Tab(usize),
    PrevTab,
    NextTab,
    PrevPage,
    NextPage,
    /// The scrollbar.
    Bar,
    /// An edge of the window (left, right, top, bottom) that resizes it.
    Edge([bool; 4]),
    /// A stack: an input, a result, an output or a catalyst. `output` is the category output's index in its
    /// view, for its tooltip.
    Stack {
        stack: Stack,
        view: Option<usize>,
        output: Option<usize>,
    },
    /// The "+" of a view, and whether it is on.
    Plus(usize, bool),
    /// Somewhere else on the window.
    Inside,
}

/// Every hit place of the page shown, in drawing order.
fn places(o: &Overlay, g: &geo::Geom) -> Vec<(Hit, [f32; 4])> {
    let mut out = Vec::new();
    let nav = nav_of(o, g);
    let [_, vy, _, vh] = g.viewport;
    // A scrolling list's slots count only where the viewport shows some of them.
    let shown = |r: &[f32; 4]| !g.scrolling || (r[1] + r[3] > vy && r[1] < vy + vh);
    if g.scrolling && geo::max_scroll(&o.views, g.content) > 0.0 {
        out.push((Hit::Bar, g.scrollbar));
    }
    for (i, rect) in &g.tabs {
        out.push((Hit::Tab(*i), *rect));
    }
    if o.tabs.len() > 1 {
        out.push((Hit::PrevTab, g.prev_tab));
        out.push((Hit::NextTab, g.next_tab));
    }
    let pages = geo::pages(&o.views, g.content, nav);
    if pages.len() > 1 {
        out.push((Hit::PrevPage, g.prev_page));
        out.push((Hit::NextPage, g.next_page));
    }
    for (stack, rect) in o.catalysts.iter().zip(&g.catalysts) {
        let hit = Hit::Stack {
            stack: stack.clone(),
            view: None,
            output: None,
        };
        out.push((hit, *rect));
    }
    let page = o.page.min(pages.len().saturating_sub(1));
    let Some(pieces) = pages.get(page) else {
        return out;
    };
    for (n, at) in geo::layout(&o.views, pieces, g.content, nav) {
        let piece = &pieces[n];
        let view = &o.views[piece.view];
        let placed = geo::place(view, piece, at, g.content[2]);
        for (part, rect) in placed.slots.into_iter().filter(|(_, r)| shown(r)) {
            let (stack, output) = match part {
                geo::Part::Input(i) => (view.slots.get(i).and_then(|s| cycled(s)), None),
                geo::Part::Result(i) => (view.results.get(i), Some(i)),
            };
            if let Some(stack) = stack {
                let hit = Hit::Stack {
                    stack: stack.clone(),
                    view: Some(piece.view),
                    output: output.filter(|_| view.layout == Layout::Chance),
                };
                out.push((hit, rect));
            }
        }
        if let (Some(rect), Some(on)) = (placed.button, view.action)
            && shown(&rect)
        {
            out.push((Hit::Plus(piece.view, on), rect));
        }
    }
    out
}

/// Whether a hit is on the recipes (what a scrolling list's viewport clips), not on the window's frame.
fn on_recipes(hit: &Hit) -> bool {
    matches!(hit, Hit::Stack { view: Some(_), .. } | Hit::Plus(..))
}

/// The hit under `point`, if the point is on the recipe screen at all.
pub(crate) fn hit(o: &Overlay, gui: [f32; 2], point: [f32; 2]) -> Option<Hit> {
    let g = geom(o, gui);
    let places = places(o, &g);
    // The recipes a scrolling list clips can't be pointed at outside its viewport.
    let in_view = !g.scrolling || jei::inside(point, g.viewport);
    let found = places
        .iter()
        .rev()
        .find(|(h, r)| jei::inside(point, *r) && (in_view || !on_recipes(h)));
    if let Some((hit, _)) = found {
        return Some(hit.clone());
    }
    if jei::inside(point, g.panel) {
        // The window's edges resize it.
        let [x, y, w, h] = g.panel;
        let edge = [
            point[0] < x + geo::EDGE,
            point[0] >= x + w - geo::EDGE,
            point[1] < y + geo::EDGE,
            point[1] >= y + h - geo::EDGE,
        ];
        return Some(if edge.iter().any(|e| *e) {
            Hit::Edge(edge)
        } else {
            Hit::Inside
        });
    }
    (!g.catalysts.is_empty() && jei::inside(point, g.catalyst_panel)).then_some(Hit::Inside)
}

/// What the drawing needs of the game besides the draw context.
struct Painter<'a> {
    host: &'a dyn ClientHost,
    draw: &'a mut dyn DrawContext,
}

impl Painter<'_> {
    fn sprite_size(&self, name: &str) -> Option<[f32; 2]> {
        let id = Identifier::parse(name).ok()?;
        let region = self.host.gui_sprite(&id)?;
        Some([region.size[0] as f32, region.size[1] as f32])
    }

    /// Sprite `name` stretched over `r` as a nine-slice with corners of `border` pixels; false when the sprite
    /// isn't there.
    fn nine(&mut self, name: &str, r: [f32; 4], border: f32) -> bool {
        let name = format!("{SPRITES}{name}");
        let Some([sw, sh]) = self.sprite_size(&name) else {
            return false;
        };
        let [x, y, w, h] = r;
        let b = border.min(w / 2.0).min(h / 2.0);
        let cols = [
            (0.0, b, x, b),
            (b, sw - 2.0 * b, x + b, w - 2.0 * b),
            (sw - b, b, x + w - b, b),
        ];
        let rows = [
            (0.0, b, y, b),
            (b, sh - 2.0 * b, y + b, h - 2.0 * b),
            (sh - b, b, y + h - b, b),
        ];
        for (sx, sw_, dx, dw) in cols {
            for (sy, sh_, dy, dh) in rows {
                if dw > 0.0 && dh > 0.0 {
                    self.draw
                        .sprite(&name, [sx, sy, sw_, sh_], [dx, dy, dw, dh], None);
                }
            }
        }
        true
    }

    /// Icon `name` at its own size, centred in `r`.
    fn icon(&mut self, name: &str, r: [f32; 4]) -> bool {
        let name = format!("{SPRITES}icons/{name}");
        let Some([w, h]) = self.sprite_size(&name) else {
            return false;
        };
        let at = [
            (r[0] + (r[2] - w) / 2.0).floor(),
            (r[1] + (r[3] - h) / 2.0).floor(),
            w,
            h,
        ];
        self.draw.sprite(&name, [0.0, 0.0, w, h], at, None)
    }

    /// The recipe window: JEI's beveled grey panel.
    fn frame(&mut self, r: [f32; 4]) {
        let [x, y, w, h] = r;
        if self.nine("gui_background", r, 4.0) {
            return;
        }
        let d = &mut *self.draw;
        d.fill([x - 1.0, y - 1.0, w + 2.0, h + 2.0], OUTLINE);
        d.fill(r, PANEL_BG);
        d.fill([x, y, w - 1.0, 1.0], PANEL_LIGHT);
        d.fill([x, y, 1.0, h - 1.0], PANEL_LIGHT);
        d.fill([x + 1.0, y + h - 1.0, w - 1.0, 1.0], PANEL_DARK);
        d.fill([x + w - 1.0, y + 1.0, 1.0, h - 1.0], PANEL_DARK);
    }

    /// JEI's category and page lines: a translucent dark band between the arrows.
    fn bar(&mut self, r: [f32; 4]) {
        self.draw.fill(r, 0x30_00_00_00);
    }

    /// The light rounded box JEI draws behind each recipe.
    fn recipe_background(&mut self, r: [f32; 4]) {
        if self.nine("single_recipe_background", r, 4.0) {
            return;
        }
        let [x, y, w, h] = r;
        let d = &mut *self.draw;
        d.fill([x + 1.0, y, w - 2.0, h], 0xFF_A8_A8_A8);
        d.fill([x, y + 1.0, w, h - 2.0], 0xFF_A8_A8_A8);
        d.fill([x + 1.0, y + 1.0, w - 2.0, h - 2.0], 0xFF_B8_B8_B8);
    }

    /// JEI's shapeless icon, half size, in the top right corner of a recipe's background.
    fn shapeless(&mut self, back: [f32; 4]) {
        let name = format!("{SPRITES}icons/shapeless_icon");
        if let Some([w, h]) = self.sprite_size(&name) {
            let at = [back[0] + back[2] - 11.0, back[1] + 2.0, 9.0, 9.0];
            self.draw.sprite(&name, [0.0, 0.0, w, h], at, None);
        }
    }

    fn slot(&mut self, r: [f32; 4]) {
        if self.nine("slot", r, 1.0) {
            return;
        }
        let [x, y, w, h] = r;
        let d = &mut *self.draw;
        d.fill(r, SLOT_BG);
        d.fill([x, y, w - 1.0, 1.0], SLOT_DARK);
        d.fill([x, y, 1.0, h - 1.0], SLOT_DARK);
        d.fill([x + 1.0, y + h - 1.0, w - 1.0, 1.0], PANEL_LIGHT);
        d.fill([x + w - 1.0, y + 1.0, 1.0, h - 1.0], PANEL_LIGHT);
    }

    fn button(&mut self, r: [f32; 4], label: &str, on: bool, hovered: bool) {
        let sprite = match (on, hovered) {
            (false, _) => "button_disabled",
            (true, true) => "button_highlight",
            (true, false) => "button_enabled",
        };
        let color = if on { 0xFF_FF_FF_FF } else { 0xFF_A0_A0_A0 };
        if self.nine(sprite, r, 3.0) {
            // JEI's arrows and transfer icon on its buttons; any other label as text.
            let drawn = match label {
                "<" => self.icon("arrow_previous", r),
                ">" => self.icon("arrow_next", r),
                "+" => self.icon("recipe_transfer", r),
                _ => false,
            };
            if drawn {
                if !on {
                    self.draw.fill(r, 0x60_00_00_00);
                }
                return;
            }
        } else {
            let fill = match (on, hovered) {
                (false, _) => 0xFF_6E_6E_6E,
                (true, true) => 0xFF_8C_9A_D8,
                (true, false) => 0xFF_A0_A0_A0,
            };
            self.draw.fill(r, OUTLINE);
            self.draw
                .fill([r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], fill);
        }
        let w = self.draw.text_width(label);
        let at = [(r[0] + (r[2] - w) / 2.0).round(), r[1] + (r[3] - 8.0) / 2.0];
        self.draw.text(label, at, color, true);
    }

    /// JEI's scrollbar: its track and the marker.
    fn scrollbar(&mut self, area: [f32; 4], visible: f32, hidden: f32, offset: f32) {
        if !self.nine("scrollbar_background", area, 6.0) {
            self.draw.fill(area, 0xFF_00_00_00);
            self.draw.fill(
                [area[0] + 1.0, area[1] + 1.0, area[2] - 2.0, area[3] - 2.0],
                SLOT_BG,
            );
        }
        let marker = geo::marker(area, visible, hidden, offset);
        if !self.nine("scrollbar_marker", marker, 2.0) {
            self.slot(marker);
        }
    }

    fn stack(&mut self, stack: &Stack, r: [f32; 4]) {
        let item = st().tables.item_stack(stack);
        if let Some(item) = item {
            let count = (stack.count > 1).then(|| stack.count.to_string());
            self.draw
                .gui_draw_item_with(&item, [r[0] + 1.0, r[1] + 1.0], count.as_deref());
        }
    }

    /// The text centred on `x`, cut with "..." to fit `max`.
    fn centred(&mut self, text: &str, x: f32, y: f32, max: f32, argb: u32) {
        let mut text = text.to_owned();
        while self.draw.text_width(&text) > max && text.chars().count() > 3 {
            let keep: String = text.chars().take(text.chars().count() - 4).collect();
            text = format!("{keep}...");
        }
        let w = self.draw.text_width(&text);
        self.draw
            .text(&text, [(x - w / 2.0).round(), y], argb, true);
    }

    /// A tab: JEI's 24 px sprite, the category's icon centred on it.
    fn tab(&mut self, o: &Overlay, i: usize, rect: [f32; 4]) {
        let sprite = if i == o.tab {
            "tab_selected"
        } else {
            "tab_unselected"
        };
        if !self.nine(sprite, rect, 4.0) {
            self.frame(rect);
            if i != o.tab {
                self.draw.fill(rect, 0x30_00_00_00);
            }
        }
        if let Some(icon) = o.tabs.get(i) {
            // `stack` insets by one: a 16 px icon centred on the 24 px tab starts 4 px in.
            self.stack(icon, [rect[0] + 3.0, rect[1] + 3.0, 18.0, 18.0]);
        }
    }
}

/// Wraps the notes to the window's width (198 at most, less its margins) the first time the window draws: the
/// geometry counts their lines.
fn wrap_notes(draw: &dyn DrawContext) {
    let views = {
        let state = st();
        match &state.overlay {
            Some(o) if !o.wrapped => o.views.clone(),
            _ => return,
        }
    };
    let views: Vec<View> = views
        .into_iter()
        .map(|mut v| {
            if !v.note.is_empty() {
                v.note = jei::wrap(&v.note, 168.0, |t| draw.text_width(t)).join("\n");
            }
            v
        })
        .collect();
    if let Some(o) = st().overlay.as_mut() {
        o.views = views;
        o.wrapped = true;
    }
}

/// The recipe screen, after the whole GUI: over the screen it opened on (`screen_render_post` said where that
/// is this frame).
pub(crate) fn gui_render_post(host: &dyn ClientHost, draw: &mut dyn DrawContext) {
    let at = st().overlay_at.take();
    let Some((rect, mouse)) = at else {
        return;
    };
    if !host.on(jei::ID) {
        return;
    }
    wrap_notes(draw);
    let Some(o) = st().overlay.clone() else {
        return;
    };
    let lang = crate::glue::lang(host);
    let gui = draw.size();
    let g = geom(&o, gui);
    let hit = hit(&o, gui, mouse);
    let mut p = Painter { host, draw };
    // The container screen under it is not drawn (the recipe_screen mixins): the window replaces it, over the
    // vanilla background. Over the world (JEI's own screen) the world dims as under any screen.
    if o.own_screen {
        p.draw.fill([0.0, 0.0, gui[0], gui[1]], DIM);
    }
    let _ = rect;
    // The window, then its tabs over its top edge (JEI draws them after it).
    p.frame(g.panel);
    for (i, r) in &g.tabs {
        p.tab(&o, *i, *r);
    }
    if !g.catalysts.is_empty() {
        let [x, y, w, h] = g.catalyst_panel;
        if !p.nine("catalyst_tab", g.catalyst_panel, 4.0) {
            p.frame(g.catalyst_panel);
        }
        // One slot background behind every catalyst, 5 px in; the items are 16 px.
        if !p.nine("slot", [x + 5.0, y + 5.0, w - 10.0, h - 10.0], 1.0) {
            p.slot([x + 5.0, y + 5.0, w - 10.0, h - 10.0]);
        }
        for (stack, r) in o.catalysts.iter().zip(&g.catalysts) {
            p.stack(stack, [r[0] - 1.0, r[1] - 1.0, 18.0, 18.0]);
        }
    }
    // JEI's two bars: the category's name, then the page, each between its arrows.
    let name = o.names.get(o.tab).cloned().unwrap_or_default();
    p.bar(g.title_bar);
    p.centred(&name, g.title[0], g.title[1], g.title_bar[2] - 4.0, WHITE);
    p.button(g.prev_tab, "<", o.tabs.len() > 1, hit == Some(Hit::PrevTab));
    p.button(g.next_tab, ">", o.tabs.len() > 1, hit == Some(Hit::NextTab));
    let nav = nav_of(&o, &g);
    let pages = geo::pages(&o.views, g.content, nav);
    let page = o.page.min(pages.len().saturating_sub(1));
    // A one-page category shows its whole subtitle there; more pages, the page too.
    let line = match (o.subtitle.is_empty(), pages.len() > 1) {
        (true, _) => lang.page_of(page, pages.len()),
        (false, false) => o.subtitle.clone(),
        (false, true) => format!("{}  {}", o.subtitle, lang.page_of(page, pages.len())),
    };
    // Scrolling has no page row: the scrollbar takes its place.
    if !g.scrolling {
        p.bar(g.page_bar);
        p.centred(
            &line,
            g.page_text[0],
            g.page_text[1],
            g.page_bar[2] - 4.0,
            WHITE,
        );
        p.button(
            g.prev_page,
            "<",
            pages.len() > 1,
            hit == Some(Hit::PrevPage),
        );
        p.button(
            g.next_page,
            ">",
            pages.len() > 1,
            hit == Some(Hit::NextPage),
        );
    }
    // A scrolling list shows through its viewport: what is outside it is cut.
    let clipped = g.scrolling && p.draw.push_clip(g.viewport).is_ok();
    if let Some(pieces) = pages.get(page) {
        for (n, at) in geo::layout(&o.views, pieces, g.content, nav) {
            let piece = &pieces[n];
            let view = &o.views[piece.view];
            let placed = geo::place(view, piece, at, g.content[2]);
            // JEI's recipe background behind each recipe, the shapeless mark in its corner.
            let [w, h] = geo::size(view, piece, g.content[2]);
            let back = [
                at[0] - geo::PAD,
                at[1] - geo::PAD,
                w + 2.0 * geo::PAD,
                h + 2.0 * geo::PAD,
            ];
            p.recipe_background(back);
            let shapeless = jei::Lang::is_shapeless_note(&view.note);
            if shapeless {
                p.shapeless(back);
            }
            for (part, r) in &placed.slots {
                p.slot(*r);
                let stack = match part {
                    geo::Part::Input(i) => view.slots.get(*i).and_then(|s| cycled(s)),
                    geo::Part::Result(i) => view.results.get(*i),
                };
                if let Some(stack) = stack {
                    p.stack(stack, *r);
                }
                if let (geo::Part::Result(i), Layout::Chance) = (part, view.layout)
                    && let Some(c) = view.chances.get(*i)
                {
                    let mark = if c.notes.is_empty() { "" } else { "*" };
                    let x = r[0] + geo::SLOT + 2.0;
                    let chance = format!("{}{mark}", jei::percent(c.chance));
                    p.draw.text(&chance, [x, r[1] + 1.0], TEXT, false);
                    let range = jei::count_range(c.min, c.max);
                    if !range.is_empty() {
                        p.draw.text(&range, [x, r[1] + 10.0], 0xFF_70_70_70, false);
                    }
                }
            }
            if let Some([x, y, w, h]) = placed.arrow {
                let mid = y + h / 2.0;
                p.draw.fill([x, mid - 2.0, w - 6.0, 4.0], 0xFF_8B_8B_8B);
                let tip = [
                    [x + w - 7.0, y],
                    [x + w, mid],
                    [x + w, mid],
                    [x + w - 7.0, y + h],
                ];
                p.draw.quad(tip, 0xFF_8B_8B_8B);
            }
            if let (Some(r), Some(on)) = (placed.button, view.action) {
                let hovered = hit == Some(Hit::Plus(piece.view, on));
                p.button(r, "+", on, hovered);
            }
            if !view.note.is_empty() && !shapeless && piece.outputs.start == 0 {
                for (k, text) in view.note.split('\n').enumerate() {
                    let at = [placed.notes[0], placed.notes[1] + k as f32 * geo::LINE];
                    p.draw.text(text, at, 0xFF_50_50_50, false);
                }
            }
        }
    }
    if clipped {
        let _ = p.draw.pop_clip();
    }
    if g.scrolling {
        let max = geo::max_scroll(&o.views, g.content);
        if max > 0.0 {
            p.scrollbar(g.scrollbar, g.viewport[3], max, bar_offset(&o, max));
        }
    }
    // The tooltip of what the pointer is on.
    let lines: Vec<String> = match &hit {
        Some(Hit::Stack {
            stack,
            view,
            output,
        }) => stack_lines(host, &o, stack, *view, *output),
        Some(Hit::Plus(_, on)) => vec![format!("§f{}", lang.transfer_hint(*on))],
        Some(Hit::Tab(i)) => (o.names.get(*i))
            .map(|n| vec![format!("§f{n}")])
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    if !lines.is_empty() {
        let texts: Vec<Text> = (lines.iter())
            .map(|l| {
                host.text_from_legacy(l)
                    .unwrap_or_else(|_| Text::literal(l.as_str()))
            })
            .collect();
        let _ = p.draw.gui_tooltip(&texts, mouse);
    }
}

/// A stack's tooltip on the recipe screen: its name, and a category output's chance, count and notes.
fn stack_lines(
    host: &dyn ClientHost,
    o: &Overlay,
    stack: &Stack,
    view: Option<usize>,
    output: Option<usize>,
) -> Vec<String> {
    // A variant by its own name (Potion of Swiftness), an item by the viewer's.
    let variant = (!stack.components.is_empty())
        .then(|| st().tables.item_stack(stack))
        .flatten();
    let name = match variant {
        Some(item) => host.hover_name(&item),
        None => st().name(&stack.item),
    };
    let mut lines = vec![format!("§f{name}")];
    if let (Some(v), Some(i)) = (view, output)
        && let Some(c) = o.views.get(v).and_then(|v| v.chances.get(i))
    {
        let range = jei::count_range(c.min, c.max);
        let range = if range.is_empty() {
            String::new()
        } else {
            format!(" {range}")
        };
        lines.push(format!("§e{}{range}", jei::percent(c.chance)));
        lines.extend(c.notes.iter().map(|n| format!("§7{n}")));
    }
    lines
}
