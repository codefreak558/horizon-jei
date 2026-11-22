# Changelog

## 0.4.0 — 2026-11-22 (SDK port)

- **A native mod on Horizon's mod SDK.** The mixin bundle (`mixins/jei/`, spliced into the game as
  `crate::game` modules) is gone; current launchers refuse that format. The mod is now one crate
  implementing `mod_api::Hooks`, pinned to Horizon `2b77287` (Nightly 2026-11-22) in
  `horizon-mod.toml`'s `[native]`, built with `horizon-launcher native-build`.
- The mod id stays `jei`, so 0.3's `config/jei/jei.cfg` and `config/jei/bookmarks.txt` carry over (same
  formats). The separate content mod `content/jei/` (the GUI sprites) is folded in as `assets/jei/`.
- Ported: the item list (`panel_items` with JEI's 1.12 `PanelStyle::Classic`), search, R/U, the
  recipe screen, "+" (recipes from the player's recipe book; Shift places as many as fit), creative
  give, the bookmark bar, the settings page (now `register_setting` rows with translations in English
  and Spanish) and `/jei`.
- **JEI's recipe screen look**: centred, a category bar and a page bar between arrows, JEI's category
  names, each recipe on JEI's recipe background with the shapeless icon, "+" at its bottom right.
- **Subtypes from the registries**: every potion (bottle, splash, lingering, tipped arrow), enchanted
  book level, goat horn and ominous bottle, each searchable by its own name.
- **Brewing** from the game's data-driven `minecraft:brewing` recipes.
- **The native route** for what the SDK lacks: the list on the creative screen and the creative give of
  a variant (`[[native.mixin]]`). The mod is therefore made for exactly the pinned build.
- Removed: the `R: Recipes (n)` / `U: Uses (n)` tooltip lines and their setting (not a JEI feature;
  `jei.cfg`'s `hints=` is still read and kept), and 0.3's "docked" view (not a JEI feature).
- Not in this mod: the kit's loot drops, ore generation and villager trades, and the "JEI kit" API
  (`docs/handover-kit-extras.md`).
- The tests moved to `src/tests/` (a native package may hold Rust only under `src/`). CI runs fmt,
  clippy and tests against the pinned SDK, and packs and checks the `.hmod` with
  `cargo horizon-mod build` / `check`; `compatibility.json`, `tools/package.py`, `proof.py` and
  the old Nightly compile job are removed.
- Porting: codefreak558.

## 0.3.0 — 2026-11-14

- **Mod API & JEI Kit support.** Added public integration API (`hz_jeikit_v15_jei`, `hz_kit_layer_jei::register`, `hz_kit_window_jei`, `hz_kit_math_jei`, `hz_kit_loot_jei`, `hz_kit_keys_jei`, `hz_kit_panel_jei`) and the optional proc-macro crate (`mixins/jei/optional/`), allowing third-party mods (AE2, Elevator, Iron Furnaces, etc.) to register custom tabs, cards, and recipes seamlessly.
- **"+" works with no recipe-book unlock.** It moves a recipe in through
  `native_transfer_recipe`, Horizon's viewer transfer, so it no longer needs the
  recipe in the player's recipe book (JEI never did). It is on for a crafting
  recipe whenever a grid it fits is open: the inventory's 2x2, the crafting
  table, or a mod's crafting menu (a crafting station); grey elsewhere and for a
  3x3 recipe over the 2x2. After a "+" the recipe screen closes on the grid.
- **Data pack recipes.** The viewer asks the server for its recipe types
  (`datapack_sync`) and rebuilds when they arrive (`recipes_received`), so a
  data pack's own recipes are listed and their "+" names what the server has.
- **JEI's own recipe screen.** R, U, a click in the list or on a bookmark now
  open a window centred over the open screen, as JEI's `RecipesGui`: the tabs
  above it, the tab's catalysts in a column at its left (a mod's crafting block
  among the crafting table's), the tab's name and page arrows, as many recipes
  a page as fit, a "+" on each crafting recipe, tooltips with the R/U counts.
  Left click on a stack shows its recipes, right click its uses; R, U and A
  work over its stacks. Escape and Backspace go back a page and close it at the
  first; the wheel turns pages. The screen under it keeps its menu, grid and
  cursor stack, and "+" fills that screen. The item list beside it and the
  bookmarks still work. The 0.2 panel view stays on the settings page
  ("Recipes: in the item panel").
- **Categories of chances.** The core takes recipe categories whose outputs
  drop with a chance (a fishing loot table, a sieve): their own tab, catalysts,
  inputs, each output's chance (one decimal, "<0.1%"), count range and notes,
  long ones over several pages. The game feeds them once Horizon has a hook
  for mods to declare them.
- Needs hook points 411 (`native_gui_draw_item_with`, `native_gui_tooltip`).


## 0.2.1 — World switch cache fix

A patch release fixing viewer cache persistence across world transitions when JEI is disabled:

- **Fix:** Clear cached viewer on world unload even while JEI is disabled. Previously, `sync` returned early when `native_on(jei::ID)` was false before the unload branch, leaving the old world's viewer in memory across world switches.
- **Session management:** Encapsulate viewer lifecycle into `Session` in `runtime.rs`, ensuring the cache is unconditionally cleared upon leaving a world.
- **Regression test:** Cover the disable -> switch worlds -> re-enable lifecycle sequence.

Verified with `cargo test` (44) and `native.py check` on Nightly `6560641`.

## 0.2.0 — Overlay and bookmarks

The overlay JEI is known for, on Horizon's screen hooks. It needs hook points
of version 313 or later (`native_item_stack`); it was tested on Nightly
2026-11-07, `6560641`.

- **Bookmarks.** The **A** key bookmarks the stack under the pointer, over a
  panel item or any slot of the container screen. The bar sits at the screen's
  top left corner, two columns of twelve: the left button opens a stack's
  recipes, the right one its uses, shift-click or the middle button takes the
  bookmark away, and dragging a slot moves it. Each slot shows the stack, and
  hovering it draws the item's name and its actions.
- **Kept between sessions.** `config/jei/bookmarks.txt` holds the bar and
  `config/jei/jei.cfg` the rest; `/jei config` says where they are.
- **Item tooltips** now say `R: Recipes (12)` and `U: Uses (3)`, and mark the
  bookmarked stack with a star. They go into every tooltip the game shows,
  the panel's included, and count exactly: a stack with components counts the
  recipes the tables name for those components.
- **Subtypes**: every `(item, components)` pair the recipe tables name is an
  item of its own in the list, so the potions and the tipped arrows are
  there like JEI's subtypes. Up to eight variants per item.
- **The list's order**: by name or by id, from the settings page or
  `/jei sort name|id`, kept in the config file.
- New commands and settings: `/jei sort`, `/jei bookmarks clear`, `/jei config`;
  the settings page's buttons now cover the order, the tooltip hints and the bar.
- Protections: a click on the bar is the mod's, never the world's, and the
  overlay draws only over container screens.

Verified with `cargo test` (43) and `native.py check` on Nightly `6560641`.

## 0.1.0 — JEI Core

The first playable viewer, on Horizon's native hook points (version 2). No
Nightly is pinned: it builds on any Horizon whose hook points are version 2 or
later.

- The item panel beside container screens: every item the game knows, paged by
  the game, with a search box.
- JEI's search grammar: words, `"quoted phrases"`, `-exclusions`, `|`
  alternatives and the `@namespace` / `#tag` prefixes (`$tooltip` and
  `%creative tab` wait for their data).
- R shows an item's recipes, U its uses, over the panel or any slot of the
  container screen. A click on a panel item does the same.
- Recipe pages with one tab per station (crafting, furnace, blast furnace,
  smoker, campfire, stonecutter, smithing, brewing), the game's five layouts,
  cooking time and XP, and a Back button that walks the history.
- A recipe slot opens that stack's recipes.
- "+" places a recipe the recipe book holds into the open grid, greyed until
  the book has it (no shift yet: the panel's action event carries no
  modifiers).
- "Click gives items" (Options > Mods > JEI, or `/jei give`): in creative a
  click gives one, a right click a stack; in survival the same click opens the
  recipes.
- English and Spanish UI (`Language` on the mod's page). Item names always come
  from the game in the player's language.
- `/jei` reports the table sizes.

Verified on Nightly `1da354b` with `cargo test` (31), `native.py check` and an
offscreen `proof.py` run: row and settings, search, tabs, transfer, history and
an empty search.