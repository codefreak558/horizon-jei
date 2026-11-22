//! The add-on API: how another mod gives JEI a category of its own (JER's mob drops, a sieve, a fishing rod's
//! loot) and fills it with recipes whose outputs drop with a chance.
//!
//! The root of the crate re-exports the three functions, so a dependent mod calls `jei::register_category`,
//! `jei::set_chance_recipes` and `jei::clear_category` (declare `jei = { version = "^0.5", optional = true }`
//! under `[dependencies]` in `horizon-mod.toml` and guard the calls with `mod_api::if_installed!`).
//!
//! Everything here is plain owned data (`String`, `f32`, `Vec`), with no reference into JEI's internals, and
//! safe under any order of calls: before or after the viewer is built, from any client hook, any number of
//! times. Unknown items and ids never panic; an unknown item simply matches nothing. The viewer picks up a
//! change on the next client tick.

use crate::runtime::{Category, ChanceRecipe, Layout, Stack};
use std::sync::{Mutex, MutexGuard};

/// Most recipes JEI keeps per category; the rest of a longer set is dropped.
pub const RECIPES_PER_CATEGORY: usize = 20_000;

#[derive(Default)]
struct Entry {
    id: String,
    /// The mod called `register_category`; recipes set before that wait, hidden.
    registered: bool,
    title: String,
    icon: String,
    layout: Layout,
    recipes: Vec<ChanceRecipe>,
}

#[derive(Default)]
struct Registry {
    entries: Vec<Entry>,
    /// Raised by every change, so the viewer knows when to read again.
    generation: u64,
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry {
    entries: Vec::new(),
    generation: 0,
});

fn registry() -> MutexGuard<'static, Registry> {
    REGISTRY.lock().unwrap_or_else(|e| e.into_inner())
}

impl Registry {
    fn entry(&mut self, id: &str) -> &mut Entry {
        let at = match self.entries.iter().position(|e| e.id == id) {
            Some(at) => at,
            None => {
                self.entries.push(Entry {
                    id: id.to_owned(),
                    ..Entry::default()
                });
                self.entries.len() - 1
            }
        };
        &mut self.entries[at]
    }
}

/// Declares a category: a tab of the recipe screen with `title`, drawn with `icon_item` (an item id such as
/// `minecraft:rotten_flesh`) and laid out as `layout`.
///
/// Idempotent per `id`: calling it again updates the title, icon and layout and keeps the recipes. Tabs follow
/// the order of the first registration, after the vanilla stations. An empty `id` is ignored. Only
/// [`Layout::Chance`] is drawn today; any other layout is shown as `Chance`.
pub fn register_category(id: &str, title: &str, icon_item: &str, layout: Layout) {
    if id.is_empty() {
        return;
    }
    let mut reg = registry();
    let entry = reg.entry(id);
    entry.registered = true;
    entry.title = title.to_owned();
    entry.icon = icon_item.to_owned();
    entry.layout = layout;
    reg.generation += 1;
}

/// Replaces the recipes of a category (a mod sends its full set each time). R over an item lists the recipes
/// whose outputs contain it; U lists those whose inputs contain it. A recipe with no output and no input is
/// kept but never found. Recipes set for an id that is not registered (yet) wait until it is.
pub fn set_chance_recipes(category_id: &str, mut recipes: Vec<ChanceRecipe>) {
    if category_id.is_empty() {
        return;
    }
    recipes.truncate(RECIPES_PER_CATEGORY);
    let mut reg = registry();
    reg.entry(category_id).recipes = recipes;
    reg.generation += 1;
}

/// Empties a category and keeps it registered; call it when the world is left. An unknown id does nothing.
pub fn clear_category(category_id: &str) {
    let mut reg = registry();
    if let Some(entry) = reg.entries.iter_mut().find(|e| e.id == category_id) {
        entry.recipes.clear();
        reg.generation += 1;
    }
}

/// How many times the registry changed, so a reader refreshes only when it must.
pub(crate) fn generation() -> u64 {
    registry().generation
}

/// The registered categories, in registration order, as the viewer takes them, with the generation they are
/// from. A category with no recipes is left out.
pub(crate) fn snapshot() -> (u64, Vec<Category>) {
    let reg = registry();
    let categories = (reg.entries.iter())
        .filter(|e| e.registered && !e.recipes.is_empty())
        .map(|e| Category {
            id: e.id.clone(),
            title: e.title.clone(),
            subtitle: String::new(),
            icon: Stack::item(&e.icon),
            layout: e.layout,
            catalysts: Vec::new(),
            recipes: e.recipes.clone(),
        })
        .collect();
    (reg.generation, categories)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{Chance, Context, Event, Item, Lang, Out, Viewer};

    /// The registry is process-wide, so the tests that use it take turns.
    static LOCK: Mutex<()> = Mutex::new(());

    fn turn() -> MutexGuard<'static, ()> {
        let guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        *registry() = Registry::default();
        guard
    }

    fn drop_of(item: &str, chance: f32) -> ChanceRecipe {
        ChanceRecipe {
            inputs: Vec::new(),
            outputs: vec![Chance::new(Stack::item(item), chance)],
            notes: Vec::new(),
        }
    }

    #[test]
    fn register_is_idempotent_and_keeps_recipes() {
        let _t = turn();
        register_category(
            "jer:mobs",
            "Mob Drops",
            "minecraft:zombie_spawn_egg",
            Layout::Chance,
        );
        set_chance_recipes("jer:mobs", vec![drop_of("minecraft:rotten_flesh", 0.67)]);
        register_category("jer:mobs", "Mobs", "minecraft:bone", Layout::Chance);
        let (_, cats) = snapshot();
        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].title, "Mobs");
        assert_eq!(cats[0].icon.item, "minecraft:bone");
        assert_eq!(cats[0].recipes.len(), 1);
    }

    #[test]
    fn recipes_replace_and_clear_keeps_the_category() {
        let _t = turn();
        register_category("a", "A", "minecraft:stone", Layout::Chance);
        set_chance_recipes("a", vec![drop_of("minecraft:diamond", 1.0); 3]);
        set_chance_recipes("a", vec![drop_of("minecraft:emerald", 1.0)]);
        let (_, cats) = snapshot();
        assert_eq!(cats[0].recipes.len(), 1);
        assert_eq!(
            cats[0].recipes[0].outputs[0].stack.item,
            "minecraft:emerald"
        );
        clear_category("a");
        assert!(snapshot().1.is_empty());
        set_chance_recipes("a", vec![drop_of("minecraft:diamond", 1.0)]);
        assert_eq!(snapshot().1.len(), 1, "still registered after a clear");
    }

    #[test]
    fn any_call_order_is_safe() {
        let _t = turn();
        // Recipes before the category, unknown ids, empty ids.
        set_chance_recipes("late", vec![drop_of("mod:unknown_item", 0.5)]);
        assert!(snapshot().1.is_empty(), "hidden until registered");
        clear_category("nobody");
        register_category("", "x", "y", Layout::Chance);
        set_chance_recipes("", Vec::new());
        register_category("late", "Late", "mod:also_unknown", Layout::Chance);
        let (_, cats) = snapshot();
        assert_eq!(cats.len(), 1);
        assert_eq!(cats[0].recipes.len(), 1);
    }

    #[test]
    fn generation_moves_with_every_change() {
        let _t = turn();
        let g = generation();
        register_category("a", "A", "minecraft:stone", Layout::Chance);
        let g1 = generation();
        assert!(g1 > g);
        clear_category("a");
        assert!(generation() > g1);
        let after = generation();
        clear_category("nobody");
        assert_eq!(generation(), after, "an unknown id changes nothing");
    }

    #[test]
    fn long_sets_are_cut() {
        let _t = turn();
        register_category("a", "A", "minecraft:stone", Layout::Chance);
        set_chance_recipes("a", vec![ChanceRecipe::default(); RECIPES_PER_CATEGORY + 5]);
        assert_eq!(snapshot().1[0].recipes.len(), RECIPES_PER_CATEGORY);
    }

    #[test]
    fn lookup_by_output_input_and_layout() {
        let _t = turn();
        register_category("jer:mobs", "Mob Drops", "minecraft:bone", Layout::Chance);
        let mut recipe = drop_of("minecraft:rotten_flesh", 0.67);
        recipe.inputs = vec![vec![Stack::item("minecraft:zombie_spawn_egg")]];
        recipe.notes = vec!["minecraft:zombie".into()];
        recipe.outputs[0].notes = vec!["Only when killed by a player".into()];
        set_chance_recipes("jer:mobs", vec![recipe]);
        let item = |id: &str| Item {
            id: id.into(),
            name: id.into(),
            tags: Vec::new(),
        };
        let items = vec![
            item("minecraft:rotten_flesh"),
            item("minecraft:zombie_spawn_egg"),
        ];
        let (mut viewer, _) = Viewer::new(items, Vec::new(), Lang::En);
        viewer.set_categories(snapshot().1);
        let ctx = Context {
            recipe_key: "key.keyboard.r",
            uses_key: "key.keyboard.u",
            give: false,
            creative: false,
            grid: 0,
        };
        let open = |viewer: &mut Viewer, id: &str, uses: bool| {
            viewer.handle(
                Event::Open {
                    stack: Stack::item(id),
                    uses,
                },
                &ctx,
            )
        };
        let out = open(&mut viewer, "minecraft:rotten_flesh", false);
        let Out::Recipes { views, names, .. } = &out[0] else {
            panic!("no page: {out:?}");
        };
        assert_eq!(names, &["Mob Drops"]);
        assert_eq!(views[0].layout, Layout::Chance);
        assert_eq!(views[0].chances[0].notes, ["Only when killed by a player"]);
        assert_eq!(crate::runtime::percent(views[0].chances[0].chance), "67.0%");
        // By input.
        let out = open(&mut viewer, "minecraft:zombie_spawn_egg", true);
        assert!(matches!(&out[0], Out::Recipes { views, .. } if views.len() == 1));
        // An item nothing names says so instead of panicking.
        let out = open(&mut viewer, "mod:nothing", false);
        assert!(matches!(&out[0], Out::Say(_)));
        // After a clear and a refresh the tab is gone.
        clear_category("jer:mobs");
        viewer.set_categories(snapshot().1);
        let out = open(&mut viewer, "minecraft:rotten_flesh", false);
        assert!(matches!(&out[0], Out::Say(_)));
    }
}
