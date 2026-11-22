//! The edges between the mod API and the viewer (`runtime`): the game's items and recipes in, the panel, give
//! and "+" out, and the overlay (tooltips, the bookmark bar and its clicks) wired to the viewer.
//!
//! Hooks that only borrow the game (`&dyn ClientHost`: screens, input, tooltips) leave actions in
//! `State::pending`; `client_tick_post` (`&mut dyn ClientHost`) runs them. The state's lock is never held across a
//! host or draw call: a host call can fire another of this mod's hooks.
use std::collections::{BTreeSet, HashMap};
use std::sync::{LazyLock, Mutex, MutexGuard};

use mod_api::serde_json::Value;
use mod_api::{
    ClientHost, ConfigPath, ItemKey, ItemStack, PanelEvent, PanelStyle, ScreenView, Text,
};

use crate::runtime::{
    self as jei, Bookmarks, Config, Context, Counts, Event, Ingredient, Item, Kind, Out, Recipe,
    Session, Sort, Stack, Toggle, Viewer,
};
use crate::window::{self, Overlay};

/// Variants one item adds to the list (potions, tipped arrows).
const VARIANTS_PER_ITEM: usize = 8;
/// Ticks between two reads while the game has no recipes yet (a world still joining).
const RETRY_TICKS: u32 = 40;
/// The recipe types read from the game, and the kind each one's recipes are.
const RECIPE_TYPES: [&str; 7] = [
    "minecraft:crafting",
    "minecraft:smelting",
    "minecraft:blasting",
    "minecraft:smoking",
    "minecraft:campfire_cooking",
    "minecraft:stonecutting",
    "minecraft:smithing",
];

/// The viewer while a world is open, and what the overlay keeps.
pub(crate) struct State {
    session: Session,
    pub(crate) tables: Tables,
    bookmarks: Bookmarks,
    pub(crate) config: Config,
    /// The config and the bookmarks have been read.
    loaded: bool,
    /// Actions a hook that only borrows the game leaves to the tick.
    pending: Vec<Event>,
    /// Stacks the overlay's keys bookmark at the next tick.
    to_bookmark: Vec<Stack>,
    /// The bookmark under the left button, while it is down.
    drag: Option<Drag>,
    /// The screen drawn last: its id and menu type, for the "+" (which grid is open). None once a tick passes
    /// without one.
    screen: Option<(&'static str, Option<String>)>,
    screen_seen: bool,
    /// Where this frame's screen is and the pointer, for the recipe screen `gui_render_post` draws.
    pub(crate) overlay_at: Option<([f32; 4], [f32; 2])>,
    /// Build the viewer again (the server's recipes arrived, or the first read had none).
    rebuild: bool,
    retry: u32,
    /// JEI's recipe screen, while it is open.
    pub(crate) overlay: Option<Overlay>,
    /// The last press went to the recipe screen: its release and drags too.
    overlay_pressed: bool,
    /// The GUI's size at the last screen drawn, for the input hooks' hit tests.
    pub(crate) gui: [f32; 2],
    /// The panel's window width now (`PanelStyle::Classic`), to set it only when it changes.
    panel_window: Option<u16>,
    /// Why the last write of a file failed, for `/jei config`.
    write_error: Option<String>,
}

/// How the viewer's item ids map to the game's keys.
#[derive(Default)]
pub(crate) struct Tables {
    keys: HashMap<String, ItemKey>,
    ids: HashMap<ItemKey, String>,
    /// The game's id of each crafting recipe. The viewer's `book_id` of a crafting recipe is its index here:
    /// the recipe book knows only unlocked recipes, so the book's own id is asked when "+" is pressed.
    recipe_ids: Vec<String>,
}

impl Tables {
    pub(crate) fn key(&self, id: &str) -> Option<ItemKey> {
        self.keys.get(id).copied()
    }

    pub(crate) fn id(&self, key: ItemKey) -> Option<&str> {
        self.ids.get(&key).map(String::as_str)
    }

    /// A viewer stack as the game's (components are not carried yet: see docs/port-sdk.md).
    pub(crate) fn item_stack(&self, stack: &Stack) -> Option<ItemStack> {
        ItemStack::new(self.key(&stack.item)?, stack.count.clamp(1, 99))
    }

    fn panel_stack(&self, stack: &Stack) -> Option<mod_api::Stack> {
        Some(mod_api::Stack {
            item: self.key(&stack.item)?,
            count: stack.count.max(1),
            components: String::new(),
        })
    }

    /// The game's stack as the viewer's.
    fn stack_of(&self, stack: &mod_api::Stack) -> Option<Stack> {
        Some(Stack::item(self.id(stack.item)?))
    }
}

/// A bookmark being dragged to another slot.
#[derive(Clone, Copy, Debug)]
struct Drag {
    from: usize,
    /// The pointer moved to another slot: a drop reorders instead of opening.
    moved: bool,
}

impl State {
    fn new() -> Self {
        Self {
            session: Session::new(),
            tables: Tables::default(),
            bookmarks: Bookmarks::default(),
            config: Config::default(),
            loaded: false,
            pending: Vec::new(),
            to_bookmark: Vec::new(),
            drag: None,
            screen: None,
            screen_seen: false,
            overlay_at: None,
            rebuild: false,
            retry: 0,
            overlay: None,
            overlay_pressed: false,
            gui: [0.0, 0.0],
            panel_window: None,
            write_error: None,
        }
    }

    pub(crate) fn viewer(&self) -> Option<&Viewer> {
        self.session.viewer()
    }

    fn viewer_mut(&mut self) -> Option<&mut Viewer> {
        self.session.viewer_mut()
    }

    /// An item's name, its id when no viewer knows it.
    pub(crate) fn name(&self, id: &str) -> String {
        self.viewer()
            .map_or_else(|| id.to_owned(), |v| v.item_name(id))
    }

    pub(crate) fn counts(&self, stack: &Stack) -> Counts {
        self.viewer().map_or(Counts::default(), |v| {
            v.counts(&stack.item, &stack.components)
        })
    }

    pub(crate) fn bookmarked(&self, stack: &Stack) -> bool {
        self.bookmarks.position(stack).is_some()
    }
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::new()));

/// The state, recovered if a panic poisoned it.
pub(crate) fn st() -> MutexGuard<'static, State> {
    STATE.lock().unwrap_or_else(|e| e.into_inner())
}

// ------------------------------------------------------------------------------------------------- files

/// `config/jei/jei.cfg` and `config/jei/bookmarks.txt`, through the mod's own config files.
fn config_path() -> Option<ConfigPath> {
    ConfigPath::new(&format!("{}/jei.cfg", jei::ID)).ok()
}

fn bookmarks_path() -> Option<ConfigPath> {
    ConfigPath::new(&format!("{}/bookmarks.txt", jei::ID)).ok()
}

fn read(host: &dyn ClientHost, path: Option<ConfigPath>) -> Option<String> {
    let bytes = host.config_files().read(&path?).ok()?;
    String::from_utf8(bytes).ok()
}

fn write(host: &dyn ClientHost, path: Option<ConfigPath>, text: &str) {
    if let Some(path) = path {
        let error = (host.config_files().write(&path, text.as_bytes()))
            .err()
            .map(|e| format!("could not write {path:?}: {e:?}"));
        st().write_error = error;
    }
}

/// Reads the config and the bookmarks once, and shows the config on the settings page.
fn load(host: &dyn ClientHost) {
    if st().loaded {
        return;
    }
    let config = read(host, config_path()).map_or_else(Config::default, |t| Config::parse(&t));
    let bookmarks =
        read(host, bookmarks_path()).map_or_else(Bookmarks::default, |t| jei::bookmarks_parse(&t));
    {
        let mut state = st();
        state.config = config;
        state.bookmarks = bookmarks;
        state.loaded = true;
    }
    crate::settings::show(host, config);
}

/// The config now, read the first time.
pub(crate) fn config(host: &dyn ClientHost) -> Config {
    load(host);
    st().config
}

pub(crate) fn store_config(host: &dyn ClientHost, config: Config) {
    st().config = config;
    write(host, config_path(), &config.to_text());
}

fn save_bookmarks(host: &dyn ClientHost, bookmarks: &Bookmarks) {
    write(host, bookmarks_path(), &jei::bookmarks_text(bookmarks));
}

/// Where the config and the bookmarks live, for `/jei config`.
pub(crate) fn files() -> String {
    let files = format!(
        "JEI files: config/{0}/jei.cfg and config/{0}/bookmarks.txt",
        jei::ID
    );
    match st().write_error.clone() {
        Some(error) => format!("{files} (last write failed: {error})"),
        None => files,
    }
}

/// The player's language (the settings page's choice).
pub(crate) fn lang(host: &dyn ClientHost) -> jei::Lang {
    match host.setting_choice("language") {
        Some(1) => jei::Lang::Es,
        _ => jei::Lang::En,
    }
}

fn say(host: &mut dyn ClientHost, line: &str) {
    let _ = host.chat_line(Text::literal(line));
}

// ------------------------------------------------------------------------------------------------- the tables

/// The game's items and recipes, as the viewer's tables.
fn read_tables(host: &mut dyn ClientHost) -> (Tables, Vec<Item>, Vec<Recipe>) {
    let mut tables = Tables::default();
    let mut items = Vec::new();
    let mut index = HashMap::new();
    for key in host.item_keys() {
        let id = key.id(host);
        if id == "minecraft:air" {
            continue;
        }
        let name = host.hover_name(&ItemStack::of(key));
        index.insert(id.clone(), items.len() as u32);
        tables.keys.insert(id.clone(), key);
        tables.ids.insert(key, id.clone());
        items.push(Item {
            id,
            name,
            tags: Vec::new(),
        });
    }
    let mut recipes = Vec::new();
    let mut named = BTreeSet::new();
    for kind in RECIPE_TYPES {
        for r in host.recipes_of_type(kind).iter() {
            tags_named(&r.json, &mut named);
            if let Some(mut recipe) = convert(r, &index, &tables) {
                if recipe.kind.station() == jei::Station::Crafting {
                    recipe.book_id = tables.recipe_ids.len() as i32;
                    tables.recipe_ids.push(r.id.clone());
                }
                recipes.push(recipe);
            }
        }
    }
    // A tag's items, as an ingredient naming it reads them: what `#logs` searches.
    for tag in named {
        let Ok(json) = mod_api::serde_json::to_string(&format!("#{tag}")) else {
            continue;
        };
        let Ok(Some(ingredient)) = host.recipe_ingredient(&json) else {
            continue;
        };
        for id in &ingredient.order {
            if let Some(&n) = index.get(id) {
                items[n as usize].tags.push(tag.clone());
            }
        }
    }
    (tables, items, recipes)
}

/// The item tags a recipe's file names (`"#minecraft:logs"`, or `{"tag": "minecraft:logs"}`), as full names.
pub fn tags_named(json: &Value, into: &mut BTreeSet<String>) {
    match json {
        Value::String(s) => {
            if let Some(tag) = s.strip_prefix('#').filter(|t| !t.is_empty()) {
                into.insert(qualified(tag));
            }
        }
        Value::Array(list) => list.iter().for_each(|v| tags_named(v, into)),
        Value::Object(map) => {
            for (key, value) in map {
                match (key.as_str(), value) {
                    ("tag", Value::String(tag)) => {
                        into.insert(qualified(tag.trim_start_matches('#')));
                    }
                    ("pattern" | "result" | "components", _) => {}
                    _ => tags_named(value, into),
                }
            }
        }
        _ => {}
    }
}

fn qualified(id: &str) -> String {
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

/// A shaped recipe's grid from its file's `pattern` and the ingredients of its filled cells, in order (the
/// game leaves the empty cells out): `(width, height, cells)`; none when they don't match.
pub fn shaped<T: Clone + Default>(rows: &[&str], filled: &[T]) -> Option<(u32, u32, Vec<T>)> {
    let width = rows.iter().map(|r| r.chars().count()).max()?;
    let mut next = filled.iter();
    let mut cells = Vec::with_capacity(width * rows.len());
    for row in rows {
        let mut n = 0;
        for c in row.chars() {
            cells.push(if c == ' ' {
                T::default()
            } else {
                next.next()?.clone()
            });
            n += 1;
        }
        cells.extend(std::iter::repeat_n(T::default(), width - n));
    }
    next.next()
        .is_none()
        .then_some((width as u32, rows.len() as u32, cells))
}

/// A game recipe as the viewer's; none for one it has no page for (a special crafting recipe, a mod's type)
/// or that makes nothing the game knows.
fn convert(r: &mod_api::Recipe, index: &HashMap<String, u32>, tables: &Tables) -> Option<Recipe> {
    let alts = |i: &mod_api::Ingredient| -> Vec<Ingredient> {
        (i.order.iter())
            .filter_map(|id| index.get(id))
            .take(jei::STACKS_MAX)
            .map(|&item| Ingredient {
                item,
                count: 1,
                components: String::new(),
            })
            .collect()
    };
    let result = |s: &ItemStack| -> Option<Ingredient> {
        Some(Ingredient {
            item: *index.get(tables.id(s.item())?)?,
            count: s.count().max(1),
            components: String::new(),
        })
    };
    let made = |r: &mod_api::Recipe| -> Vec<Ingredient> {
        r.results.iter().filter_map(result).take(1).collect()
    };
    let mut recipe = Recipe {
        kind: Kind::CraftingShapeless,
        width: 0,
        height: 0,
        slots: Vec::new(),
        results: Vec::new(),
        cooking_time: r.cooking_time.unwrap_or(0),
        experience: r.experience.unwrap_or(0.0),
        book_id: -1,
    };
    let kind = r.kind.as_str();
    match (kind, &r.rule) {
        (
            "minecraft:stonecutting",
            Some(mod_api::RecipeRule::Stonecutting { input, result: out }),
        ) => {
            recipe.kind = Kind::Stonecutting;
            recipe.slots = vec![alts(input)];
            recipe.results = result(out).into_iter().collect();
        }
        (
            "minecraft:smithing",
            Some(mod_api::RecipeRule::SmithingTransform {
                template,
                base,
                addition,
                result: out,
            }),
        ) => {
            recipe.kind = Kind::Smithing;
            let opt = |i: &Option<mod_api::Ingredient>| i.as_ref().map_or_else(Vec::new, alts);
            recipe.slots = vec![opt(template), alts(base), opt(addition)];
            let id = out.item.as_deref().and_then(|id| index.get(id));
            recipe.results = id
                .map(|&item| Ingredient {
                    item,
                    count: out.count.max(1),
                    components: String::new(),
                })
                .into_iter()
                .collect();
        }
        ("minecraft:crafting", None) => {
            let cells: Vec<Vec<Ingredient>> = r.ingredients.iter().map(alts).collect();
            let rows: Option<Vec<&str>> = (r.json.get("pattern").and_then(Value::as_array))
                .map(|rows| rows.iter().filter_map(Value::as_str).collect());
            recipe.results = made(r);
            match rows {
                Some(rows) => {
                    let (w, h, grid) = shaped(&rows, &cells)?;
                    recipe.kind = Kind::CraftingShaped;
                    (recipe.width, recipe.height) = (w, h);
                    recipe.slots = grid;
                }
                None => recipe.slots = cells,
            }
        }
        ("minecraft:smelting", None)
        | ("minecraft:blasting", None)
        | ("minecraft:smoking", None)
        | ("minecraft:campfire_cooking", None) => {
            recipe.kind = match kind {
                "minecraft:smelting" => Kind::Smelting,
                "minecraft:blasting" => Kind::Blasting,
                "minecraft:smoking" => Kind::Smoking,
                _ => Kind::Campfire,
            };
            recipe.slots = vec![alts(r.ingredients.first()?)];
            recipe.results = made(r);
        }
        _ => return None,
    }
    let any_input = recipe.slots.iter().any(|s| !s.is_empty());
    (any_input && !recipe.results.is_empty()).then_some(recipe)
}

/// Builds the viewer once per world (and again when asked), and shows the item list in the panel.
pub(crate) fn sync(host: &mut dyn ClientHost) {
    let has_player = host.local_player().is_some();
    let enabled = host.on(jei::ID);
    let needs_build = {
        let mut state = st();
        if !std::mem::take(&mut state.screen_seen) {
            state.screen = None;
        }
        if state.rebuild && has_player && enabled {
            if state.retry > 0 {
                state.retry -= 1;
            } else {
                state.rebuild = false;
                state.session.clear();
            }
        }
        let needs_build = state.session.update(has_player, enabled);
        if !has_player {
            state.drag = None;
            state.pending.clear();
            state.overlay = None;
            state.panel_window = None;
        }
        needs_build
    };
    if !needs_build {
        return;
    }
    load(host);
    let (tables, items, recipes) = read_tables(host);
    let recipes = jei::dedupe_recipes(recipes);
    let none = recipes.is_empty();
    let found = jei::harvest_variants(&recipes, VARIANTS_PER_ITEM, jei::ITEMS_MAX);
    let lang = lang(host);
    let (mut viewer, mut out) = Viewer::with_variants(items, recipes, found, lang);
    let sort = st().config.sort;
    if sort != viewer.sort() {
        out = viewer.set_sort(sort);
    }
    {
        let mut state = st();
        state.session.set_viewer(viewer);
        state.tables = tables;
        if none {
            // A world still joining has no recipes yet: read again in a while.
            state.rebuild = true;
            state.retry = RETRY_TICKS;
        }
    }
    host.panel_style(jei::ID, panel_style(0));
    st().panel_window = Some(0);
    apply_or_say(host, out);
}

fn panel_style(window_width: u16) -> PanelStyle {
    PanelStyle::Classic {
        centred_search: true,
        window_width,
    }
}

/// The server's recipes came in: the next tick builds the viewer from them.
pub(crate) fn recipes_received() {
    let mut state = st();
    state.rebuild = true;
    state.retry = 0;
}

/// Runs what the input hooks left (a click on a bookmark, the recipe screen's buttons), once a tick.
pub(crate) fn flush(host: &mut dyn ClientHost) {
    if !host.on(jei::ID) {
        return;
    }
    let (pending, marks) = {
        let mut state = st();
        (
            std::mem::take(&mut state.pending),
            std::mem::take(&mut state.to_bookmark),
        )
    };
    for stack in marks {
        bookmark(host, stack);
    }
    for event in pending {
        dispatch(host, event);
    }
    // The item list lays out beside the recipe screen while it is open.
    let want = {
        let state = st();
        let gui = state.gui;
        state
            .overlay
            .as_ref()
            .map_or(0, |o| window::width(o, gui).ceil() as u16)
    };
    let changed = {
        let mut state = st();
        let changed = state.panel_window.is_some_and(|w| w != want);
        if changed {
            state.panel_window = Some(want);
        }
        changed
    };
    if changed {
        host.panel_style(jei::ID, panel_style(want));
    }
}

fn pending(event: Event) {
    st().pending.push(event);
}

// ------------------------------------------------------------------------------------------------- the panel

/// Whether `key` (`key.keyboard.r`) is the key bound to the mod's mapping `name`.
pub(crate) fn bound(host: &dyn ClientHost, name: &str, key: &str) -> bool {
    host.mod_key(jei::ID, name).is_some_and(|(k, _)| k == key)
}

/// The panel's events: search, clicks, keys over a stack.
pub(crate) fn panel_event(host: &mut dyn ClientHost, event: &PanelEvent) {
    if !host.on(jei::ID) {
        return;
    }
    let event = {
        let state = st();
        match event {
            PanelEvent::Search(text) => Some(Event::Search(text.clone())),
            PanelEvent::Clicked { index, right } => Some(Event::Clicked {
                index: *index as usize,
                right: *right,
            }),
            PanelEvent::Key { key, stack } => {
                state.tables.stack_of(stack).map(|stack| Event::Key {
                    key: key.clone(),
                    stack,
                })
            }
            _ => None,
        }
    };
    let Some(event) = event else {
        return;
    };
    // The bookmark key bookmarks the stack under the pointer, wherever it is: the game says which one it is.
    if let Event::Key { key, stack } = &event
        && bound(host, "bookmark", key)
    {
        bookmark(host, stack.clone());
        return;
    }
    dispatch(host, event);
}

/// Bookmarks a stack, or takes its bookmark away, and says so.
fn bookmark(host: &mut dyn ClientHost, stack: Stack) {
    load(host);
    let lang = lang(host);
    let (result, bookmarks, name) = {
        let mut state = st();
        let result = state.bookmarks.toggle(&stack);
        (result, state.bookmarks.clone(), state.name(&stack.item))
    };
    if result != Toggle::Full {
        save_bookmarks(host, &bookmarks);
    }
    let text = match result {
        Toggle::Added(_) => format!("{}: {name}", lang.bookmark_added()),
        Toggle::Removed(_) => format!("{}: {name}", lang.bookmark_removed()),
        Toggle::Full => lang.bookmarks_full().to_owned(),
    };
    say(host, &text);
}

/// Takes a bookmark away by its slot, keeping the file.
fn remove_bookmark(host: &dyn ClientHost, index: usize) {
    let bookmarks = {
        let mut state = st();
        state.drag = None;
        state
            .bookmarks
            .remove(index)
            .map(|_| state.bookmarks.clone())
    };
    if let Some(bookmarks) = bookmarks {
        save_bookmarks(host, &bookmarks);
    }
}

/// Empties the bar, keeping the file.
pub(crate) fn clear_bookmarks(host: &dyn ClientHost) {
    load(host);
    let bookmarks = {
        let mut state = st();
        state.bookmarks.clear();
        state.drag = None;
        state.bookmarks.clone()
    };
    save_bookmarks(host, &bookmarks);
}

/// The crafting grid of the screen drawn last: 0 when none takes recipes.
fn open_grid() -> u32 {
    st().screen
        .as_ref()
        .map_or(0, |(id, menu)| jei::screen_grid(id, menu.as_deref(), None))
}

fn dispatch(host: &mut dyn ClientHost, event: Event) {
    let grid = open_grid();
    let key = |name: &str, fallback: &str| {
        host.mod_key(jei::ID, name)
            .map_or_else(|| fallback.to_owned(), |(k, _)| k)
    };
    let recipe_key = key("recipe", "key.keyboard.r");
    let uses_key = key("uses", "key.keyboard.u");
    let give = host.setting_bool("give").unwrap_or(false);
    let lang = lang(host);
    let ctx = Context {
        recipe_key: &recipe_key,
        uses_key: &uses_key,
        give,
        // `give` succeeds in creative only: try, and fall back below.
        creative: give,
        grid,
    };
    let mut fallback = false;
    for out in handle(&event, &ctx, lang) {
        if let Out::Give { item, count } = &out {
            let key = st().tables.key(item);
            if key.is_none_or(|key| host.give(key, *count).is_err()) {
                fallback = true;
            }
            continue;
        }
        apply_or_say(host, out);
    }
    if fallback {
        // Survival: the same click opens the recipes, as JEI without cheat mode.
        let ctx = Context {
            give: false,
            creative: false,
            ..ctx
        };
        for out in handle(&event, &ctx, lang) {
            apply_or_say(host, out);
        }
    }
}

fn handle(event: &Event, ctx: &Context<'_>, lang: jei::Lang) -> Vec<Out> {
    let mut state = st();
    let Some(viewer) = state.viewer_mut() else {
        return Vec::new();
    };
    viewer.set_lang(lang);
    viewer.handle(event.clone(), ctx)
}

/// The viewer's size, for `/jei`; none outside a world.
pub(crate) fn counts() -> Option<(usize, usize)> {
    (st().viewer()).map(|v| (v.item_count(), v.recipe_count()))
}

/// The next order, shown in the list at once.
pub(crate) fn set_sort(host: &mut dyn ClientHost, sort: Sort) {
    let mut config = config(host);
    config.sort = sort;
    store_config(host, config);
    let out = st().viewer_mut().map(|v| v.set_sort(sort));
    if let Some(out) = out {
        apply_or_say(host, out);
    }
}

fn apply_or_say(host: &mut dyn ClientHost, out: Out) {
    if let Err(error) = apply(host, out) {
        say(host, &format!("JEI: {error}"));
    }
}

fn apply(host: &mut dyn ClientHost, out: Out) -> Result<(), String> {
    match out {
        Out::Items(stacks) => {
            let items: Vec<mod_api::Stack> = {
                let state = st();
                (stacks.iter())
                    .filter_map(|s| state.tables.panel_stack(s))
                    .take(jei::ITEMS_MAX)
                    .collect()
            };
            // An empty list sent while the game dispatches a search keeps the panel and its search box
            // (`Panel::show_items`), so a search with no match shows an empty list.
            host.panel_items(jei::ID, items);
            Ok(())
        }
        Out::Recipes {
            title,
            tabs,
            tab,
            views,
            names,
            catalysts,
            subtitle,
        } => {
            // The game's panel can't show recipe pages from the public API yet (horizon-rust/horizon#19392):
            // JEI's own recipe screen always holds them, whatever `docked` says.
            if views.is_empty() {
                close_overlay(host);
                return Ok(());
            }
            let (open, same) = {
                let state = st();
                let open = state.overlay.as_ref();
                (
                    open.map(|o| o.own_screen),
                    open.is_some_and(|o| o.title == title && o.tab == tab as usize),
                )
            };
            // Nothing open (a lookup from the world): JEI's own screen holds it.
            let own_screen = match open {
                Some(own) => own,
                None => {
                    let nothing_open = st().screen.is_none();
                    nothing_open && host.mod_screen_open(jei::ID)
                }
            };
            let mut state = st();
            let page = match &state.overlay {
                Some(o) if same => o.page,
                _ => 0,
            };
            state.overlay = Some(Overlay {
                title,
                tabs,
                names,
                tab: tab as usize,
                views,
                catalysts,
                subtitle,
                page,
                own_screen,
                wrapped: false,
            });
            Ok(())
        }
        Out::Give { item, count } => {
            let key = st()
                .tables
                .key(&item)
                .ok_or_else(|| format!("unknown item {item}"))?;
            host.give(key, count)
        }
        Out::Transfer {
            book_id: index,
            all,
        } => {
            let id = st().tables.recipe_ids.get(index.max(0) as usize).cloned();
            let book = id.and_then(|id| host.recipe_book_ids(&id).first().copied());
            let Some(book) = book else {
                return Err(lang(host).not_in_book().to_owned());
            };
            // "+" moves the ingredients (JEI's transfer); with Shift, as many sets as the items allow, through
            // the recipe book's own placing.
            if all && host.place_recipe(book, true).is_ok() {
                return Ok(());
            }
            host.transfer_recipe(book);
            Ok(())
        }
        Out::Say(text) => {
            say(host, &text);
            Ok(())
        }
    }
}

/// Closes the recipe screen (and JEI's own screen under it).
fn close_overlay(host: &mut dyn ClientHost) {
    let closed = {
        let mut state = st();
        state.overlay_pressed = false;
        state.overlay.take()
    };
    if closed.is_some_and(|o| o.own_screen) {
        host.mod_screen_close(jei::ID);
    }
}

// ------------------------------------------------------------------------------------------------- the overlay

/// The screens the bookmark bar shows on: the containers', and any screen the recipe screen is open over.
fn bar_screen(screen: &ScreenView<'_>) -> bool {
    jei::overlay_screen(screen.id) || st().overlay.is_some()
}

/// The bookmark bar over a container screen, and where the recipe screen goes this frame.
pub(crate) fn screen_render_post(
    host: &dyn ClientHost,
    draw: &mut dyn mod_api::DrawContext,
    screen: &ScreenView<'_>,
) {
    {
        let mut state = st();
        state.screen = Some((screen.id, screen.menu.map(str::to_owned)));
        state.screen_seen = true;
        state.gui = draw.size();
    }
    if !host.on(jei::ID) {
        return;
    }
    // The recipe screen itself draws in `gui_render_post`, over this screen and whatever a mod drew on it after
    // us; it needs where this one is.
    st().overlay_at = Some((screen.rect, screen.mouse));
    if !bar_screen(screen) {
        return;
    }
    load(host);
    let (config, slots, drag) = {
        let state = st();
        let slots: Vec<(Stack, Option<ItemStack>)> = (state.bookmarks.slots().iter())
            .map(|s| (s.clone(), state.tables.item_stack(s)))
            .collect();
        (state.config, slots, state.drag)
    };
    if !config.bookmarks || slots.is_empty() {
        return;
    }
    let bar = jei::bookmark_bar(draw.size()[1]);
    let count = slots.len().min(bar.capacity());
    let hovered = bar.hit(screen.mouse, count);
    // As JEI's lists: the items with no box behind them.
    for (index, (_, item)) in slots.iter().enumerate().take(count) {
        let rect = bar.slot(index);
        if hovered == Some(index) || drag.is_some_and(|d| d.from == index) {
            draw.fill(rect, 0x50FF_FFFF);
        }
        if let Some(item) = item {
            draw.item_stack(item, [rect[0] + 1.0, rect[1] + 1.0], 16.0);
        }
    }
    let Some(index) = hovered else {
        return;
    };
    let name = st().name(&slots[index].0.item);
    let lines: Vec<Text> = jei::bookmark_tooltip(&name, lang(host))
        .into_iter()
        .map(|(text, [r, g, b, _])| colored(host, &text, [r, g, b]))
        .collect();
    let _ = draw.gui_tooltip(&lines, screen.mouse);
}

/// A line in an RGB colour, as a legacy `§` code would give it (the nearest of the sixteen).
fn colored(host: &dyn ClientHost, text: &str, [r, g, b]: [u8; 3]) -> Text {
    let code = match (r, g, b) {
        (255, 255, 85) => 'e',
        (170, 170, 170) => '7',
        (85, 85, 85) => '8',
        _ => 'f',
    };
    host.text_from_legacy(&format!("§{code}{text}"))
        .unwrap_or_else(|_| Text::literal(text))
}

/// A press: the recipe screen's, else the bar's. The left button takes a bookmark (opening its recipes when
/// the release comes without a move), the right one opens its uses, Shift and the middle button take it away.
pub(crate) fn mouse_pressed(
    host: &dyn ClientHost,
    screen: &ScreenView<'_>,
    button: i32,
    modifiers: u32,
) -> bool {
    if !host.on(jei::ID) || !bar_screen(screen) {
        return false;
    }
    load(host);
    let (config, bookmarks) = {
        let state = st();
        (state.config, state.bookmarks.clone())
    };
    let hit = config
        .bookmarks
        .then(|| jei::bookmark_slots().hit(screen.mouse, bookmarks.len()))
        .flatten();
    let Some(index) = hit else {
        return overlay_pressed(screen, button, modifiers);
    };
    let stack = bookmarks.slots()[index].clone();
    match button {
        0 if modifiers & 1 == 0 => {
            st().drag = Some(Drag {
                from: index,
                moved: false,
            });
        }
        0 | 2 => remove_bookmark(host, index),
        1 => pending(Event::Open { stack, uses: true }),
        _ => {}
    }
    true
}

/// A release over the bar: a left click that never left its slot opens the stack's recipes.
pub(crate) fn mouse_released(host: &dyn ClientHost, screen: &ScreenView<'_>, button: i32) -> bool {
    if !host.on(jei::ID) {
        return false;
    }
    if std::mem::take(&mut st().overlay_pressed) {
        return true;
    }
    if !bar_screen(screen) || button != 0 {
        return false;
    }
    let Some(drag) = st().drag.take() else {
        return false;
    };
    if !drag.moved {
        let stack = st().bookmarks.get(drag.from).cloned();
        if let Some(stack) = stack {
            pending(Event::Open { stack, uses: false });
        }
    }
    true
}

/// Dragging a bookmark over another slot moves it there.
pub(crate) fn mouse_dragged(host: &dyn ClientHost, screen: &ScreenView<'_>, button: i32) -> bool {
    if !host.on(jei::ID) {
        return false;
    }
    if st().overlay_pressed {
        return true;
    }
    if !bar_screen(screen) || button != 0 {
        return false;
    }
    let (drag, count) = {
        let state = st();
        (state.drag, state.bookmarks.len())
    };
    let Some(mut drag) = drag else {
        return false;
    };
    let Some(to) = jei::bookmark_slots().hit(screen.mouse, count) else {
        return false;
    };
    if to == drag.from {
        return true;
    }
    let moved = {
        let mut state = st();
        state
            .bookmarks
            .move_slot(drag.from, to)
            .then(|| state.bookmarks.clone())
    };
    if let Some(bookmarks) = moved {
        save_bookmarks(host, &bookmarks);
        drag.from = to;
        drag.moved = true;
        st().drag = Some(drag);
    }
    true
}

/// A press on the recipe screen: it takes every press on its window, and the presses on the screen under it
/// (it is modal), but not the item list beside the screen or the bookmarks.
fn overlay_pressed(screen: &ScreenView<'_>, button: i32, modifiers: u32) -> bool {
    let shift = modifiers & 1 != 0;
    let mut state = st();
    let gui = state.gui;
    let Some(o) = state.overlay.as_ref() else {
        return false;
    };
    let (hit, tabs, tab, pages) = (
        window::hit(o, gui, screen.mouse),
        o.tabs.len(),
        o.tab,
        window::pages(o, gui),
    );
    // Outside the window, the item list (right of the window) still takes clicks; the screen under it doesn't.
    let beside = screen.mouse[0] >= (gui[0] + window::width(o, gui)) / 2.0;
    let Some(hit) = hit else {
        if beside {
            return false;
        }
        state.overlay_pressed = true;
        return true;
    };
    state.overlay_pressed = true;
    match hit {
        window::Hit::Tab(i) => state.pending.push(Event::Tab(i)),
        window::Hit::PrevTab if tabs > 0 => state.pending.push(Event::Tab((tab + tabs - 1) % tabs)),
        window::Hit::NextTab if tabs > 0 => state.pending.push(Event::Tab((tab + 1) % tabs)),
        window::Hit::PrevPage | window::Hit::NextPage => {
            if let Some(o) = state.overlay.as_mut() {
                let page = o.page.min(pages - 1);
                o.page = if hit == window::Hit::NextPage {
                    (page + 1) % pages
                } else {
                    (page + pages - 1) % pages
                };
            }
        }
        window::Hit::Stack { stack, .. } if button == 0 || button == 1 => {
            state.pending.push(Event::Open {
                stack,
                uses: button == 1,
            });
        }
        window::Hit::Plus(view, true) if button == 0 => {
            // JEI moves the items and goes back to the screen they went to.
            state.pending.push(Event::Action(view, shift));
            state.pending.push(Event::Close);
        }
        _ => {}
    }
    true
}

/// The wheel over the recipe screen turns its pages.
pub(crate) fn mouse_scrolled(
    host: &dyn ClientHost,
    screen: &ScreenView<'_>,
    scroll: [f64; 2],
) -> bool {
    if !host.on(jei::ID) || scroll[1] == 0.0 {
        return false;
    }
    let mut state = st();
    let gui = state.gui;
    let Some(o) = state.overlay.as_mut() else {
        return false;
    };
    if window::hit(o, gui, screen.mouse).is_none() {
        return false;
    }
    let pages = window::pages(o, gui);
    let page = o.page.min(pages - 1);
    o.page = if scroll[1] < 0.0 {
        (page + 1).min(pages - 1)
    } else {
        page.saturating_sub(1)
    };
    true
}

/// Keys on the recipe screen: Escape and Backspace go back a page (closing it at the first), and the viewer's
/// keys (recipes, uses, bookmark) act on the stack under the pointer.
pub(crate) fn key_pressed(host: &dyn ClientHost, screen: &ScreenView<'_>, key: &str) -> bool {
    if !host.on(jei::ID) {
        return false;
    }
    let hit = {
        let state = st();
        let gui = state.gui;
        state
            .overlay
            .as_ref()
            .map(|o| window::hit(o, gui, screen.mouse))
    };
    let Some(hit) = hit else {
        return false;
    };
    if key == "key.keyboard.escape" || (key == "key.keyboard.backspace" && hit.is_some()) {
        pending(Event::Back);
        return true;
    }
    let (recipe, uses, mark) = (
        bound(host, "recipe", key),
        bound(host, "uses", key),
        bound(host, "bookmark", key),
    );
    if !(recipe || uses || mark) || hit.is_none() {
        return false;
    }
    // Over the window, its keys never reach the slots hidden under it.
    if let Some(window::Hit::Stack { stack, .. }) = hit {
        if mark {
            st().to_bookmark.push(stack);
        } else {
            pending(Event::Open { stack, uses });
        }
    }
    true
}

/// The screen under the recipe screen closed (or JEI's own did): so does the recipe screen, and the viewer
/// forgets its history.
pub(crate) fn screen_closing(host: &dyn ClientHost) {
    if !host.on(jei::ID) {
        return;
    }
    let open = {
        let mut state = st();
        match state.overlay.as_mut() {
            Some(o) => {
                // JEI's own screen is closing: nothing to close after it.
                o.own_screen = false;
                true
            }
            None => false,
        }
    };
    if open {
        pending(Event::Close);
    }
}

/// Adds the viewer's lines to any item's tooltip: what makes it, what uses it and whether it is bookmarked.
pub(crate) fn tooltip(host: &dyn ClientHost, stack: &ItemStack, lines: &mut Vec<(Text, u32)>) {
    if !host.on(jei::ID) {
        return;
    }
    load(host);
    let (counts, marked, hints) = {
        let state = st();
        let Some(id) = state.tables.id(stack.item()) else {
            return;
        };
        let stack = Stack::item(id);
        (
            state.counts(&stack),
            state.bookmarked(&stack),
            state.config.hints,
        )
    };
    let counts = if hints { counts } else { Counts::default() };
    let added = jei::tooltip_lines(counts, marked, lang(host));
    if added.is_empty() {
        return;
    }
    if !lines.is_empty() {
        lines.push((Text::literal(""), u32::MAX));
    }
    for (text, [r, g, b, a]) in added {
        lines.push((Text::literal(text), u32::from_le_bytes([r, g, b, a])));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shaped_pattern_fills_its_grid() {
        let (w, h, grid) = shaped(&["# ", " #"], &[1, 2]).unwrap();
        assert_eq!((w, h), (2, 2));
        assert_eq!(grid, [1, 0, 0, 2]);
        // Short rows are padded; too few or too many ingredients refuse.
        assert_eq!(shaped(&["##", "#"], &[1, 2, 3]).unwrap().2, [1, 2, 3, 0]);
        assert_eq!(shaped(&["###"], &[1, 2]), None);
        assert_eq!(shaped(&["#"], &[1, 2]), None);
    }

    #[test]
    fn tags_are_read_from_a_recipe_file() {
        let json: Value = mod_api::serde_json::from_str(
            r##"{"pattern": ["#"], "key": {"#": "#logs", "X": {"tag": "wool"}},
                "result": {"id": "minecraft:stick", "components": {"x": "#nope"}}}"##,
        )
        .unwrap();
        let mut named = BTreeSet::new();
        tags_named(&json, &mut named);
        assert_eq!(
            named.into_iter().collect::<Vec<_>>(),
            ["minecraft:logs", "minecraft:wool"]
        );
    }
}
