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
    geo::fitted(gui, o.tabs.len(), o.tab, o.catalysts.len(), &o.views)
}

/// The window's width, for the item list to lay out beside it.
pub(crate) fn width(o: &Overlay, gui: [f32; 2]) -> f32 {
    geom(o, gui).panel[2]
}

/// How many pages the selected tab has (one at least).
pub(crate) fn pages(o: &Overlay, gui: [f32; 2]) -> usize {
    geo::pages(&o.views, geom(o, gui).content).len().max(1)
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
    for (i, rect) in &g.tabs {
        out.push((Hit::Tab(*i), *rect));
    }
    if o.tabs.len() > 1 {
        out.push((Hit::PrevTab, g.prev_tab));
        out.push((Hit::NextTab, g.next_tab));
    }
    let pages = geo::pages(&o.views, g.content);
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
    for (n, at) in geo::layout(&o.views, pieces, g.content) {
        let piece = &pieces[n];
        let view = &o.views[piece.view];
        let placed = geo::place(view, piece, at, g.content[2]);
        for (part, rect) in placed.slots {
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
        if let (Some(rect), Some(on)) = (placed.button, view.action) {
            out.push((Hit::Plus(piece.view, on), rect));
        }
    }
    out
}

/// The hit under `point`, if the point is on the recipe screen at all.
pub(crate) fn hit(o: &Overlay, gui: [f32; 2], point: [f32; 2]) -> Option<Hit> {
    let g = geom(o, gui);
    let places = places(o, &g);
    if let Some((hit, _)) = places.iter().rev().find(|(_, r)| jei::inside(point, *r)) {
        return Some(hit.clone());
    }
    let on = jei::inside(point, g.panel)
        || (!g.catalysts.is_empty() && jei::inside(point, g.catalyst_panel));
    on.then_some(Hit::Inside)
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
        if self.nine("gui_background", [x - 1.0, y - 1.0, w + 2.0, h + 2.0], 4.0) {
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

    /// A recessed bar, as JEI's category and page lines.
    fn bar(&mut self, r: [f32; 4]) {
        let [x, y, w, h] = r;
        let d = &mut *self.draw;
        d.fill(r, SLOT_BG);
        d.fill([x, y, w - 1.0, 1.0], SLOT_DARK);
        d.fill([x, y, 1.0, h - 1.0], SLOT_DARK);
        d.fill([x + 1.0, y + h - 1.0, w - 1.0, 1.0], PANEL_LIGHT);
        d.fill([x + w - 1.0, y + 1.0, 1.0, h - 1.0], PANEL_LIGHT);
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
            .text(&text, [(x - w / 2.0).round(), y], argb, false);
    }

    fn tab(&mut self, o: &Overlay, i: usize, rect: [f32; 4]) {
        let selected = i == o.tab;
        let r = if selected {
            [rect[0], rect[1], rect[2] - 1.0, rect[3] + 3.0]
        } else {
            [rect[0], rect[1] + 2.0, rect[2] - 1.0, rect[3] - 2.0]
        };
        let sprite = if selected {
            "tab_selected"
        } else {
            "tab_unselected"
        };
        if self.nine(sprite, r, 4.0) {
            if selected {
                self.draw
                    .fill([r[0] + 3.0, r[1] + r[3] - 5.0, r[2] - 6.0, 5.0], PANEL_BG);
            }
        } else {
            self.frame(r);
            if !selected {
                self.draw.fill(r, 0x30_00_00_00);
            }
        }
        if let Some(icon) = o.tabs.get(i) {
            let top = if selected { r[1] + 3.0 } else { r[1] + 2.0 };
            self.stack(icon, [r[0] + 3.0, top, 18.0, 18.0]);
        }
    }
}

/// Wraps the notes to the window's width (196 at most, less its margins) the first time the window draws: the
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
    // The screen under it dims, as under a vanilla screen opened over another.
    p.draw.fill(rect, DIM);
    // Tabs, as JEI's: the others behind the window, the selected one over its edge, joined to it.
    for (i, r) in g.tabs.iter().filter(|(i, _)| *i != o.tab) {
        p.tab(&o, *i, *r);
    }
    p.frame(g.panel);
    for (i, r) in g.tabs.iter().filter(|(i, _)| *i == o.tab) {
        p.tab(&o, *i, *r);
    }
    if !g.catalysts.is_empty() {
        let [x, y, w, h] = g.catalyst_panel;
        if !p.nine("catalyst_tab", [x - 1.0, y - 1.0, w + 2.0, h + 2.0], 4.0) {
            p.frame(g.catalyst_panel);
        }
        for (stack, r) in o.catalysts.iter().zip(&g.catalysts) {
            p.slot(*r);
            p.stack(stack, *r);
        }
    }
    // JEI's two bars: the category's name, then the page, each between its arrows.
    let name = o.names.get(o.tab).cloned().unwrap_or_default();
    p.bar(g.title_bar);
    p.centred(&name, g.title[0], g.title[1], g.title_bar[2] - 4.0, WHITE);
    p.button(g.prev_tab, "<", o.tabs.len() > 1, hit == Some(Hit::PrevTab));
    p.button(g.next_tab, ">", o.tabs.len() > 1, hit == Some(Hit::NextTab));
    let pages = geo::pages(&o.views, g.content);
    let page = o.page.min(pages.len().saturating_sub(1));
    // A one-page category shows its whole subtitle there; more pages, the page too.
    let line = match (o.subtitle.is_empty(), pages.len() > 1) {
        (true, _) => lang.page_of(page, pages.len()),
        (false, false) => o.subtitle.clone(),
        (false, true) => format!("{}  {}", o.subtitle, lang.page_of(page, pages.len())),
    };
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
    if let Some(pieces) = pages.get(page) {
        for (n, at) in geo::layout(&o.views, pieces, g.content) {
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
    // The tooltip of what the pointer is on.
    let lines: Vec<String> = match &hit {
        Some(Hit::Stack {
            stack,
            view,
            output,
        }) => stack_lines(&o, stack, *view, *output),
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
    o: &Overlay,
    stack: &Stack,
    view: Option<usize>,
    output: Option<usize>,
) -> Vec<String> {
    let name = st().name(&stack.item);
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
