//! JEI's config window, in the look of MezzConfig's GUI (what JEI's settings are on Fabric): a window with a
//! category list on its left, a search box over the rows of the open category, and a description box under
//! both. It is drawn over the open screen (or over JEI's own screen when nothing is open), after the whole GUI,
//! and is modal while it is open. Everything is in GUI pixels, so it scales with the game's GUI scale.
use std::sync::{LazyLock, Mutex, MutexGuard};

use mod_api::ClientHost;
use mod_api::DrawContext;

use crate::glue;
use crate::runtime::{self as jei, Sort};

const OUTLINE: u32 = 0xFF_00_00_00;
const PANEL_BG: u32 = 0xFF_C6_C6_C6;
const PANEL_LIGHT: u32 = 0xFF_FF_FF_FF;
const PANEL_DARK: u32 = 0xFF_55_55_55;
const LIST_BG: u32 = 0xFF_55_55_55;
const ROW_BG: u32 = 0xFF_6B_6B_6B;
const ROW_HOVER: u32 = 0xFF_7A_7A_7A;
const SIDE_BG: u32 = 0xFF_B4_B4_B4;
const SIDE_ON: u32 = 0xFF_8E_8E_8E;
const BOX_BG: u32 = 0xFF_20_20_20;
const BUTTON: u32 = 0xFF_8B_8B_8B;
const BUTTON_HOVER: u32 = 0xFF_9F_AB_E0;
const DESC_BG: u32 = 0xFF_22_22_30;
const WHITE: u32 = 0xFF_FF_FF_FF;
const GREY: u32 = 0xFF_B0_B0_B0;
const DIM: u32 = 0xA0_10_10_10;

const ROW: f32 = 24.0;
const MIN_SIZE: [f32; 2] = [300.0, 200.0];
/// MezzConfig's default window.
const DEFAULT_SIZE: [f32; 2] = [380.0, 300.0];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Check,
    Choice(&'static [&'static str]),
    Number { step: u32 },
}

struct Row {
    id: &'static str,
    label: &'static str,
    help: &'static str,
    kind: Kind,
    default: u32,
}

struct Category {
    name: &'static str,
    help: &'static str,
    rows: &'static [Row],
}

const CATEGORIES: [Category; 5] = [
    Category {
        name: "Lists",
        help: "Options shared by the ingredient and bookmark lists.",
        rows: &[
            Row {
                id: "navigation",
                label: "Navigation Mode",
                help: "Recipes turn pages, or scroll under a scrollbar.",
                kind: Kind::Choice(&["Paged", "Scrolling"]),
                default: 0,
            },
            Row {
                id: "sort",
                label: "Ingredient List Sorting Order",
                help: "How the ingredient list is ordered.",
                kind: Kind::Choice(&["Mod Name", "Item Name", "Item ID"]),
                default: 0,
            },
        ],
    },
    Category {
        name: "Recipe Window",
        help: "The size of the recipe window. Its edges can also be dragged.",
        rows: &[
            Row {
                id: "recipe_width",
                label: "Recipe Window Width",
                help: "The recipe window's width, from 198.",
                kind: Kind::Number { step: 10 },
                default: 198,
            },
            Row {
                id: "recipe_height",
                label: "Recipe Window Height",
                help: "The recipe window's height, from 175.",
                kind: Kind::Number { step: 10 },
                default: 350,
            },
        ],
    },
    Category {
        name: "Bookmarks",
        help: "The bookmark bar at the screen's corner.",
        rows: &[Row {
            id: "bookmarks",
            label: "Show Bookmark Overlay",
            help: "Shows the bookmark bar on screens with the item list.",
            kind: Kind::Check,
            default: 1,
        }],
    },
    Category {
        name: "Cheating",
        help: "Giving items from the list.",
        rows: &[Row {
            id: "give",
            label: "Enable Cheat Mode",
            help: "In creative a click gives one item, a right click a stack.",
            kind: Kind::Check,
            default: 0,
        }],
    },
    Category {
        name: "General",
        help: "Options for the mod itself.",
        rows: &[Row {
            id: "language",
            label: "Language",
            help: "The language of JEI's own text. Item names follow the game's.",
            kind: Kind::Choice(&["English", "Español"]),
            default: 0,
        }],
    },
];

#[derive(Clone)]
struct Ui {
    want_open: bool,
    open: bool,
    /// JEI opened a screen of its own for it (nothing was open).
    own: bool,
    /// The window closed from a hook: the tick closes the screen.
    closing: bool,
    cat: usize,
    search: String,
    focus: bool,
    scroll: f32,
    size: [f32; 2],
    mouse: [f32; 2],
    gui: [f32; 2],
    /// The grip's press: where the pointer took it and the size then.
    resize: Option<([f32; 2], [f32; 2])>,
    sort: Option<Sort>,
}

static UI: LazyLock<Mutex<Ui>> = LazyLock::new(|| {
    Mutex::new(Ui {
        want_open: false,
        open: false,
        own: false,
        closing: false,
        cat: 0,
        search: String::new(),
        focus: false,
        scroll: 0.0,
        size: DEFAULT_SIZE,
        mouse: [0.0, 0.0],
        gui: [854.0, 480.0],
        resize: None,
        sort: None,
    })
});

fn ui() -> MutexGuard<'static, Ui> {
    UI.lock().unwrap_or_else(|e| e.into_inner())
}

/// The window's parts for a GUI and window size.
#[derive(Clone, Copy, Debug)]
struct Lay {
    win: [f32; 4],
    tab: [f32; 4],
    side: [f32; 4],
    search: [f32; 4],
    clear: [f32; 4],
    apply: [f32; 4],
    list: [f32; 4],
    desc: [f32; 4],
    grip: [f32; 4],
}

fn lay(gui: [f32; 2], size: [f32; 2]) -> Lay {
    let w = size[0].clamp(MIN_SIZE[0].min(gui[0]), (gui[0] - 12.0).max(MIN_SIZE[0].min(gui[0])));
    let h = size[1].clamp(MIN_SIZE[1].min(gui[1]), (gui[1] - 12.0).max(MIN_SIZE[1].min(gui[1])));
    let x = ((gui[0] - w) / 2.0).floor();
    let y = ((gui[1] - h) / 2.0).floor();
    let side_w = (w * 0.29).clamp(80.0, 140.0).floor();
    let desc_h = 56.0;
    let desc = [x + 4.0, y + h - desc_h - 4.0, w - 8.0, desc_h];
    let body_top = y + 26.0;
    let body_bottom = desc[1] - 4.0;
    let left = x + 4.0 + side_w + 6.0;
    let apply = [x + w - 24.0, body_top, 20.0, 20.0];
    let clear = [apply[0] - 22.0, body_top, 20.0, 20.0];
    Lay {
        win: [x, y, w, h],
        tab: [x - 25.0, y + 6.0, 28.0, 28.0],
        side: [x + 4.0, body_top, side_w, body_bottom - body_top],
        search: [left, body_top, clear[0] - 2.0 - left, 20.0],
        clear,
        apply,
        list: [left, body_top + 24.0, x + w - 4.0 - left, body_bottom - body_top - 24.0],
        desc,
        grip: [x + w - 10.0, y + h - 10.0, 10.0, 10.0],
    }
}

/// The rows shown: the open category's, or every row whose label has the search in it.
fn shown(cat: usize, search: &str) -> Vec<(usize, usize)> {
    let search = search.trim().to_lowercase();
    let mut out = Vec::new();
    for (c, category) in CATEGORIES.iter().enumerate() {
        if search.is_empty() && c != cat {
            continue;
        }
        for (r, row) in category.rows.iter().enumerate() {
            if search.is_empty() || row.label.to_lowercase().contains(&search) {
                out.push((c, r));
            }
        }
    }
    out
}

fn max_scroll(u: &Ui, l: &Lay) -> f32 {
    let rows = shown(u.cat, &u.search).len() as f32;
    (rows * ROW + 4.0 - l.list[3]).max(0.0)
}

/// The row `i`'s rectangle, its widget and its reset button.
fn row_rects(u: &Ui, l: &Lay, i: usize) -> ([f32; 4], [f32; 4], [f32; 4]) {
    let [lx, ly, lw, _] = l.list;
    let r = [lx + 2.0, ly + 2.0 + i as f32 * ROW - u.scroll, lw - 12.0, 22.0];
    let reset = [r[0] + r[2] - 18.0, r[1] + 3.0, 16.0, 16.0];
    let wide = 92.0;
    let widget = [reset[0] - 4.0 - wide, r[1] + 1.0, wide, 20.0];
    (r, widget, reset)
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Part {
    Row,
    Widget,
    Down,
    Up,
    Reset,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Hit {
    Inside,
    Category(usize),
    Search,
    Clear,
    Apply,
    Grip,
    Row(usize, Part),
}

fn inside(p: [f32; 2], r: [f32; 4]) -> bool {
    jei::inside(p, r)
}

fn hit(u: &Ui, p: [f32; 2]) -> Option<Hit> {
    let l = lay(u.gui, u.size);
    if inside(p, l.grip) {
        return Some(Hit::Grip);
    }
    if !inside(p, l.win) && !inside(p, l.tab) {
        return None;
    }
    if inside(p, l.apply) {
        return Some(Hit::Apply);
    }
    if inside(p, l.clear) {
        return Some(Hit::Clear);
    }
    if inside(p, l.search) {
        return Some(Hit::Search);
    }
    for i in 0..CATEGORIES.len() {
        if inside(p, side_rect(&l, i)) {
            return Some(Hit::Category(i));
        }
    }
    if inside(p, l.list) {
        for (i, (c, r)) in shown(u.cat, &u.search).into_iter().enumerate() {
            let (row, widget, reset) = row_rects(u, &l, i);
            if !inside(p, row) {
                continue;
            }
            let kind = CATEGORIES[c].rows[r].kind;
            let part = if inside(p, reset) {
                Part::Reset
            } else if inside(p, widget) {
                match kind {
                    Kind::Number { .. } if p[0] < widget[0] + 20.0 => Part::Down,
                    Kind::Number { .. } if p[0] >= widget[0] + widget[2] - 20.0 => Part::Up,
                    _ => Part::Widget,
                }
            } else {
                Part::Row
            };
            return Some(Hit::Row(i, part));
        }
    }
    Some(Hit::Inside)
}

fn side_rect(l: &Lay, i: usize) -> [f32; 4] {
    [l.side[0] + 1.0, l.side[1] + 1.0 + i as f32 * 22.0, l.side[2] - 2.0, 21.0]
}

// ------------------------------------------------------------------------------------------------- values

fn flag(on: bool) -> &'static str {
    if on { "true" } else { "false" }
}

fn value(host: &dyn ClientHost, id: &str) -> u32 {
    let c = glue::config(host);
    match id {
        "navigation" => u32::from(c.scrolling),
        "sort" => match c.sort {
            Sort::Mod => 0,
            Sort::Name => 1,
            Sort::Id => 2,
        },
        "recipe_width" => c.recipe_width,
        "recipe_height" => c.recipe_height,
        "bookmarks" => u32::from(c.bookmarks),
        "give" => u32::from(host.setting_bool("give").unwrap_or(false)),
        "language" => host.setting_choice("language").map_or(0, |n| n as u32),
        _ => 0,
    }
}

/// Sets a row's value: the file follows, and the mod page's setting with it.
fn set(host: &dyn ClientHost, gui: [f32; 2], id: &str, v: u32) {
    let mut c = glue::config(host);
    match id {
        "navigation" => {
            c.scrolling = v == 1;
            host.set_setting(jei::ID, "navigation", &v.to_string());
        }
        "sort" => {
            host.set_setting(jei::ID, "sort", &v.to_string());
            ui().sort = Some(match v {
                1 => Sort::Name,
                2 => Sort::Id,
                _ => Sort::Mod,
            });
            return;
        }
        "recipe_width" => c.recipe_width = v.clamp(jei::screen::MIN_WIDTH as u32, (gui[0] - 12.0) as u32),
        "recipe_height" => {
            c.recipe_height = v.clamp(jei::screen::MIN_HEIGHT as u32, (gui[1] - 58.0).max(175.0) as u32);
        }
        "bookmarks" => {
            c.bookmarks = v == 1;
            host.set_setting(jei::ID, "bookmarks", flag(v == 1));
        }
        "give" => {
            host.set_setting(jei::ID, "give", flag(v == 1));
            return;
        }
        "language" => {
            host.set_setting(jei::ID, "language", &v.to_string());
            return;
        }
        _ => return,
    }
    glue::store_config(host, c);
}

fn change(host: &dyn ClientHost, gui: [f32; 2], row: &Row, part: Part, button: i32) {
    let now = value(host, row.id);
    let next = match (part, row.kind) {
        (Part::Reset, _) => row.default,
        (Part::Widget | Part::Row, Kind::Check) => 1 - now.min(1),
        (Part::Widget, Kind::Choice(options)) => {
            let n = options.len() as u32;
            if button == 1 { (now + n - 1) % n } else { (now + 1) % n }
        }
        (Part::Down, Kind::Number { step }) => now.saturating_sub(step),
        (Part::Up, Kind::Number { step }) => now + step,
        _ => return,
    };
    set(host, gui, row.id, next);
}

// ------------------------------------------------------------------------------------------------- the hooks

/// The wrench button on the item list's screens: where it is.
pub(crate) fn wrench(gui: [f32; 2]) -> [f32; 4] {
    [gui[0] - 22.0, gui[1] - 22.0, 20.0, 20.0]
}

/// Draws the wrench button.
pub(crate) fn draw_wrench(draw: &mut dyn DrawContext, mouse: [f32; 2]) {
    let r = wrench(draw.size());
    button(draw, r, inside(mouse, r));
    // A spanner: a bar with a notched head.
    let [x, y, ..] = r;
    for k in 0..9 {
        let o = k as f32 * 1.4;
        draw.fill([x + 5.0 + o, y + 13.0 - o, 3.0, 3.0], WHITE);
    }
    draw.fill([x + 12.0, y + 4.0, 4.0, 4.0], WHITE);
    draw.fill([x + 4.0, y + 12.0, 4.0, 4.0], WHITE);
}

/// Asks for the window to open (the tick opens it: it can open a screen).
pub(crate) fn request() {
    ui().want_open = true;
}

pub(crate) fn is_open() -> bool {
    ui().open
}

/// The pointer and GUI size of this frame's screen.
pub(crate) fn seen(mouse: [f32; 2], gui: [f32; 2]) {
    let mut u = ui();
    u.mouse = mouse;
    u.gui = gui;
}

/// Opens or closes what the hooks asked for.
pub(crate) fn tick(host: &mut dyn mod_api::ClientHost) {
    let (want, closing, own, sort) = {
        let mut u = ui();
        let r = (u.want_open, u.closing, u.own, u.sort.take());
        u.want_open = false;
        u.closing = false;
        r
    };
    if let Some(sort) = sort {
        glue::set_sort(host, sort);
    }
    if closing && own {
        host.mod_screen_close(jei::ID);
    }
    if want && !is_open() {
        let nothing_open = glue::nothing_open();
        let own = nothing_open && host.mod_screen_open(jei::ID);
        let mut u = ui();
        u.open = true;
        u.own = own;
        u.focus = false;
        u.scroll = 0.0;
    }
}

fn close() {
    let mut u = ui();
    u.open = false;
    u.closing = true;
    u.resize = None;
    u.focus = false;
}

/// The screen under the window (or the mod's own) closed.
pub(crate) fn closed() {
    let mut u = ui();
    u.open = false;
    u.own = false;
    u.resize = None;
}

/// A press. Open, the window takes every press; closed, the wrench button opens it (on the item list's screens).
pub(crate) fn pressed(host: &dyn ClientHost, mouse: [f32; 2], button: i32, on_list_screen: bool) -> bool {
    let u = ui().clone();
    if !u.open {
        if on_list_screen && button == 0 && inside(mouse, wrench(u.gui)) {
            request();
            return true;
        }
        return false;
    }
    let found = hit(&u, mouse);
    let mut state = ui();
    state.focus = found == Some(Hit::Search);
    match found {
        Some(Hit::Category(i)) => {
            state.cat = i;
            state.scroll = 0.0;
            state.search.clear();
        }
        Some(Hit::Clear) => state.search.clear(),
        Some(Hit::Apply) => {
            drop(state);
            close();
        }
        Some(Hit::Grip) => state.resize = Some((mouse, u.size)),
        Some(Hit::Row(i, part)) => {
            let (c, r) = shown(u.cat, &u.search)[i];
            drop(state);
            change(host, u.gui, &CATEGORIES[c].rows[r], part, button);
        }
        _ => {}
    }
    true
}

pub(crate) fn released() -> bool {
    let mut u = ui();
    u.resize = None;
    u.open
}

/// The pointer moved with a button held: the grip resizes the window, which is centred, so each pixel is two.
pub(crate) fn dragged(mouse: [f32; 2]) -> bool {
    let mut u = ui();
    if !u.open {
        return false;
    }
    if let Some((start, size)) = u.resize {
        let max = [(u.gui[0] - 12.0).max(MIN_SIZE[0]), (u.gui[1] - 12.0).max(MIN_SIZE[1])];
        u.size = [
            (size[0] + (mouse[0] - start[0]) * 2.0).clamp(MIN_SIZE[0], max[0]),
            (size[1] + (mouse[1] - start[1]) * 2.0).clamp(MIN_SIZE[1], max[1]),
        ];
    }
    true
}

pub(crate) fn scrolled(delta: f64) -> bool {
    let mut u = ui();
    if !u.open {
        return false;
    }
    let max = max_scroll(&u, &lay(u.gui, u.size));
    u.scroll = (u.scroll - delta as f32 * 20.0).clamp(0.0, max);
    true
}

/// A key: Escape closes the window, Backspace edits the search; the window takes every key while it is open.
pub(crate) fn key(key: &str) -> bool {
    let mut u = ui();
    if !u.open {
        return false;
    }
    match key {
        "key.keyboard.escape" => {
            drop(u);
            close();
        }
        "key.keyboard.backspace" if u.focus => {
            u.search.pop();
            u.scroll = 0.0;
        }
        _ => {}
    }
    true
}

/// A typed character goes into the search box when it has the focus; the window takes all of them.
pub(crate) fn char_typed(c: char) -> bool {
    let mut u = ui();
    if !u.open {
        return false;
    }
    if u.focus && !c.is_control() && u.search.chars().count() < 40 {
        u.search.push(c);
        u.scroll = 0.0;
    }
    true
}

// ------------------------------------------------------------------------------------------------- drawing

fn bevel(d: &mut dyn DrawContext, r: [f32; 4]) {
    let [x, y, w, h] = r;
    d.fill([x - 1.0, y - 1.0, w + 2.0, h + 2.0], OUTLINE);
    d.fill(r, PANEL_BG);
    d.fill([x, y, w - 1.0, 1.0], PANEL_LIGHT);
    d.fill([x, y, 1.0, h - 1.0], PANEL_LIGHT);
    d.fill([x + 1.0, y + h - 1.0, w - 1.0, 1.0], PANEL_DARK);
    d.fill([x + w - 1.0, y + 1.0, 1.0, h - 1.0], PANEL_DARK);
}

fn boxed(d: &mut dyn DrawContext, r: [f32; 4], fill: u32) {
    d.fill(r, OUTLINE);
    d.fill([r[0] + 1.0, r[1] + 1.0, r[2] - 2.0, r[3] - 2.0], fill);
}

fn button(d: &mut dyn DrawContext, r: [f32; 4], hovered: bool) {
    let [x, y, w, h] = r;
    boxed(d, r, if hovered { BUTTON_HOVER } else { BUTTON });
    d.fill([x + 1.0, y + 1.0, w - 3.0, 1.0], PANEL_LIGHT);
    d.fill([x + 1.0, y + 1.0, 1.0, h - 3.0], PANEL_LIGHT);
    d.fill([x + 2.0, y + h - 2.0, w - 3.0, 1.0], PANEL_DARK);
    d.fill([x + w - 2.0, y + 2.0, 1.0, h - 3.0], PANEL_DARK);
}

fn centred(d: &mut dyn DrawContext, text: &str, r: [f32; 4], color: u32) {
    let w = d.text_width(text);
    d.text(
        text,
        [(r[0] + (r[2] - w) / 2.0).round(), (r[1] + (r[3] - 8.0) / 2.0).round()],
        color,
        true,
    );
}

/// `text` cut to `max` pixels.
fn fitted(d: &dyn DrawContext, text: &str, max: f32) -> String {
    let mut text = text.to_owned();
    while d.text_width(&text) > max && text.chars().count() > 1 {
        text.pop();
    }
    text
}

/// The window, after the whole GUI.
pub(crate) fn draw(host: &dyn ClientHost, d: &mut dyn DrawContext) {
    let u = {
        let mut u = ui();
        if !u.open {
            return;
        }
        u.gui = d.size();
        u.clone()
    };
    let l = lay(u.gui, u.size);
    let mouse = u.mouse;
    let found = hit(&u, mouse);
    d.fill([0.0, 0.0, u.gui[0], u.gui[1]], DIM);
    bevel(d, l.win);
    // The tab on the window's left: JEI's.
    bevel(d, l.tab);
    d.fill([l.tab[0] + l.tab[2] - 3.0, l.tab[1] + 1.0, 4.0, l.tab[3] - 2.0], PANEL_BG);
    centred(d, "J", [l.tab[0], l.tab[1], l.tab[2] - 3.0, l.tab[3]], 0xFF_1F_9E_1F);
    // The title, with the quick menu's icon at its left.
    let title = "JEI Config";
    let tw = d.text_width(title);
    d.text(
        title,
        [(l.win[0] + (l.win[2] - tw) / 2.0).round(), l.win[1] + 8.0],
        0xFF_40_40_40,
        false,
    );
    // The category list.
    d.fill(l.side, SIDE_BG);
    for (i, category) in CATEGORIES.iter().enumerate() {
        let r = side_rect(&l, i);
        let on = i == u.cat && u.search.trim().is_empty();
        if on || found == Some(Hit::Category(i)) {
            d.fill(r, if on { SIDE_ON } else { 0xFF_C4_C4_C4 });
        }
        d.text(category.name, [r[0] + 4.0, r[1] + 7.0], if on { WHITE } else { 0xFF_30_30_30 }, on);
    }
    // The search box, with its clear and apply buttons.
    boxed(d, l.search, BOX_BG);
    let placeholder = u.search.is_empty() && !u.focus;
    let (text, color) = if placeholder {
        ("find configs by name".to_owned(), 0xFF_70_70_70)
    } else {
        let caret = if u.focus { "_" } else { "" };
        (format!("{}{caret}", u.search), WHITE)
    };
    let text = fitted(d, &text, l.search[2] - 8.0);
    d.text(&text, [l.search[0] + 4.0, l.search[1] + 6.0], color, false);
    button(d, l.clear, found == Some(Hit::Clear));
    centred(d, "x", l.clear, WHITE);
    button(d, l.apply, found == Some(Hit::Apply));
    centred(d, "ok", l.apply, WHITE);
    // The rows.
    d.fill(l.list, LIST_BG);
    let rows = shown(u.cat, &u.search);
    let clipped = d.push_clip(l.list).is_ok();
    let mut hovered: Option<(&str, &str)> = None;
    for (i, (c, r)) in rows.iter().enumerate() {
        let row = &CATEGORIES[*c].rows[*r];
        let (area, widget, reset) = row_rects(&u, &l, i);
        if area[1] + area[3] < l.list[1] || area[1] > l.list[1] + l.list[3] {
            continue;
        }
        let over = matches!(found, Some(Hit::Row(n, _)) if n == i);
        if over {
            hovered = Some((row.label, row.help));
        }
        d.fill(area, if over { ROW_HOVER } else { ROW_BG });
        let lines = jei::wrap(row.label, widget[0] - area[0] - 8.0, |t| d.text_width(t));
        let top = area[1] + (area[3] - lines.len().min(2) as f32 * 9.0) / 2.0;
        for (k, line) in lines.iter().take(2).enumerate() {
            d.text(line, [area[0] + 4.0, top + k as f32 * 9.0], WHITE, true);
        }
        let now = value(host, row.id);
        let part = match found {
            Some(Hit::Row(n, p)) if n == i => Some(p),
            _ => None,
        };
        match row.kind {
            Kind::Check => {
                let b = [widget[0] + widget[2] - 20.0, widget[1], 20.0, 20.0];
                boxed(d, b, BOX_BG);
                if now == 1 {
                    for k in 0..4 {
                        d.fill([b[0] + 5.0 + k as f32 * 1.5, b[1] + 9.0 + k as f32 * 1.5, 2.0, 2.0], WHITE);
                    }
                    for k in 0..7 {
                        d.fill([b[0] + 10.0 + k as f32 * 1.4, b[1] + 14.0 - k as f32 * 1.4, 2.0, 2.0], WHITE);
                    }
                }
            }
            Kind::Choice(options) => {
                boxed(d, widget, if part == Some(Part::Widget) { 0xFF_30_30_40 } else { BOX_BG });
                let label = options.get(now as usize).copied().unwrap_or("");
                let label = fitted(d, label, widget[2] - 16.0);
                d.text(&label, [widget[0] + 4.0, widget[1] + 6.0], WHITE, false);
                // A small arrowhead pointing down.
                let (ax, ay) = (widget[0] + widget[2] - 10.0, widget[1] + 8.0);
                d.fill([ax, ay, 5.0, 1.0], GREY);
                d.fill([ax + 1.0, ay + 1.0, 3.0, 1.0], GREY);
                d.fill([ax + 2.0, ay + 2.0, 1.0, 1.0], GREY);
            }
            Kind::Number { .. } => {
                boxed(d, widget, BOX_BG);
                let down = [widget[0], widget[1], 20.0, 20.0];
                let up = [widget[0] + widget[2] - 20.0, widget[1], 20.0, 20.0];
                button(d, down, part == Some(Part::Down));
                centred(d, "-", down, WHITE);
                button(d, up, part == Some(Part::Up));
                centred(d, "+", up, WHITE);
                centred(d, &now.to_string(), [widget[0] + 20.0, widget[1], widget[2] - 40.0, 20.0], WHITE);
            }
        }
        button(d, reset, part == Some(Part::Reset));
        // The reset arrow.
        d.fill([reset[0] + 5.0, reset[1] + 7.0, 6.0, 2.0], WHITE);
        d.fill([reset[0] + 4.0, reset[1] + 6.0, 1.0, 4.0], WHITE);
        d.fill([reset[0] + 3.0, reset[1] + 7.0, 1.0, 2.0], WHITE);
    }
    if clipped {
        let _ = d.pop_clip();
    }
    // The scrollbar, when the rows overflow.
    let max = max_scroll(&u, &l);
    if max > 0.0 {
        let track = [l.list[0] + l.list[2] - 8.0, l.list[1], 8.0, l.list[3]];
        boxed(d, track, BOX_BG);
        let size = (track[3] * track[3] / (track[3] + max)).max(14.0);
        let at = track[1] + (track[3] - size) * (u.scroll / max);
        d.fill([track[0] + 1.0, at, track[2] - 2.0, size], BUTTON);
    }
    // The description box: the row under the pointer, else the open category.
    boxed(d, l.desc, DESC_BG);
    let category = &CATEGORIES[u.cat.min(CATEGORIES.len() - 1)];
    let (name, help) = hovered.unwrap_or((category.name, category.help));
    d.text(name, [l.desc[0] + 6.0, l.desc[1] + 6.0], WHITE, true);
    for (k, line) in jei::wrap(help, l.desc[2] - 12.0, |t| d.text_width(t))
        .iter()
        .take(4)
        .enumerate()
    {
        d.text(line, [l.desc[0] + 6.0, l.desc[1] + 18.0 + k as f32 * 9.0], 0xFF_D8_D8_D8, false);
    }
    // The grip at the corner.
    for k in 0..3 {
        let o = 2.0 + k as f32 * 3.0;
        d.fill([l.grip[0] + l.grip[2] - 2.0, l.grip[1] + o + 2.0, 1.0, 1.0], PANEL_DARK);
        d.fill([l.grip[0] + o + 2.0, l.grip[1] + l.grip[3] - 2.0, 1.0, 1.0], PANEL_DARK);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_is_centred_and_scales_with_the_screen() {
        let l = lay([640.0, 360.0], DEFAULT_SIZE);
        assert_eq!(l.win, [130.0, 30.0, 380.0, 300.0]);
        // A small screen shrinks it to fit.
        let small = lay([320.0, 240.0], DEFAULT_SIZE);
        assert!(small.win[2] <= 308.0 && small.win[3] <= 228.0);
        assert!(small.win[0] >= 0.0 && small.win[1] >= 0.0);
    }

    #[test]
    fn the_search_lists_every_matching_row() {
        assert_eq!(shown(0, "").len(), CATEGORIES[0].rows.len());
        assert_eq!(shown(0, "sorting"), [(0, 1)]);
        assert_eq!(shown(0, "RECIPE"), [(1, 0), (1, 1)]);
        assert!(shown(0, "nothing like it").is_empty());
    }

    #[test]
    fn every_row_has_a_default_its_kind_takes() {
        for category in &CATEGORIES {
            for row in category.rows {
                if let Kind::Choice(options) = row.kind {
                    assert!((row.default as usize) < options.len(), "{}", row.id);
                }
            }
        }
    }
}
