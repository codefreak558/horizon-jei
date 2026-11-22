# Architecture

JEI for Horizon is a native mod: one Rust crate that implements `mod_api::Hooks`. The launcher compiles
it against the Horizon build pinned in `horizon-mod.toml` (`[native]`). Nothing is spliced into the game.
The mod reaches the game only through the hooks' `ClientHost` and `DrawContext`.

```
src/lib.rs       #[mod_api::horizon_mod] impl Hooks: each hook forwards to glue/window/settings
src/glue.rs      the edges: tables in (items, recipes, tags), panel/give/"+" out, the bookmark bar,
                 tooltips, input hooks, config files
src/window.rs    JEI's recipe screen (RecipesGui): drawing with DrawContext, hit tests
src/settings.rs  Options > Mods page (register_setting) and /jei (register_client_commands, command)
src/runtime.rs   the viewer: pure Rust, no game types; drives the tests
src/kit_*.rs     pure helpers kept for the kit extras (not used by the MVP)
```

## Why this split

- `runtime.rs` never names a mod API type, so the viewer's logic is tested with `cargo test`, with no
  game and no display (`src/tests/`).
- `glue.rs` and `window.rs` are the only files that touch the game. When the SDK changes, they change.
- `lib.rs` stays one line per hook.

## State and the two kinds of host

Hooks get `&dyn ClientHost` (screens, input, tooltips, drawing) or `&mut dyn ClientHost` (tick, panel
events, commands, settings). `panel_items`, `give`, `chat_line`, `mod_screen_open`, `recipe_book_ids` and
`transfer_recipe` need `&mut`. So an input hook only records what to do in `State::pending`, and
`client_tick_post` runs it.

The state is a `static Mutex<State>` (`glue::st()`, recovered if poisoned). The lock is never held
across a host or draw call: a host call can fire another of this mod's hooks (a tooltip draw fires
`item_tooltip`), and that hook would deadlock.

## Hooks used

| Hook | What it does |
|---|---|
| `register` | the settings (give, hints, bookmarks, sort, language) and the R, U, A key mappings |
| `client_tick_post` | builds the viewer once a world is open (again after `recipes_received`, or while the game has no recipes yet), runs pending actions, keeps the panel beside the recipe screen |
| `recipes_received` | the server's recipes arrived: rebuild next tick |
| `panel_event` | search, list clicks, R/U and A over any stack (`PanelEvent::Key`) |
| `item_tooltip` | the R/U counts and the bookmark mark |
| `screen_render_post` | the bookmark bar, and where the screen is for the recipe screen |
| `gui_render_post` | the recipe screen, over everything else |
| `screen_mouse_button_pressed_pre` / `_released_pre` / `_dragged_pre` / `_scrolled_pre` | the bar's clicks and drags; the recipe screen's buttons, stacks, "+", pages |
| `screen_key_pressed_pre` | Escape/Backspace go back; R, U and A over the recipe screen's stacks |
| `screen_closing` | the recipe screen closes with its screen |
| `register_client_commands` / `command` | `/jei …` |
| `setting_changed` | the page's toggles are written to `jei.cfg`; a new order re-lists |

## Data flow

```
world open ─ client_tick_post ─► item_keys + hover_name ─► Items
                                recipes_of_type(crafting, smelting, …, smithing) ─► Recipes
                                    (shaped grids from the file's pattern; tags via recipe_ingredient)
                                                   │
                                     Viewer::with_variants ─► Out::Items ─► panel_items
player searches / clicks / presses R, U, A ─► PanelEvent ─► glue::panel_event
                                                   │
                                 Viewer::handle(Event, Context) ─► Vec<Out>
                                                   │
           panel_items / recipe screen (window.rs) / give / transfer_recipe / chat_line
```

A crafting recipe's `book_id` in the viewer is an index into `Tables::recipe_ids`. "+" asks
`recipe_book_ids` for the game's recipe id when it is pressed, because the client's recipe book only
holds unlocked recipes.

## Files

`config_files()` serves the mod's own folder in `config/`: `config/jei_new/jei.cfg` (sort, hints,
bookmarks) and `config/jei_new/bookmarks.txt`. The settings page's values live in
`horizon-mods/.horizon-mod-data/jei_new/`. When `jei.cfg` is read, its values are shown on the page.
