# Porting horizon-jei to the Horizon mod SDK (.hmod)

Feasibility study (step 1). Nothing here is ported yet.

## What this was checked against

| | |
|---|---|
| Launcher | `horizon-launcher` 0.2.102 |
| Game / SDK build | Nightly 2026-11-22, `2b77287de099c9316278ba7ac4d83590fdb7ef30` (the instance's version; launcher SDK cache `native/sdk/2b77287…/`) |
| mixins | `e59da9094f0c2fdda6c627fe9eff22dec6a906f0` (from that SDK's `sdk.json`) |
| Toolchain | Rust 1.99.1 (launcher's own, `x86_64-pc-windows-gnu`) |
| API surface | `surface.json` from the same SDK (770 hooks, 222 traits, 1809 handles). Every "SDK equivalent" below is a name taken from that file |
| Internals catalog | `build-data.tar.xz` → `internals.json` (30 484 paths) from release `sdk-2b77287…`, SHA-256 checked against `sdk.json` |
| API crate | `api-source.tar.xz` from the same release (SHA-256 `ace895c5…`, matches `sdk.json`), unpacked to `.horizon/sdk/api/`, the path `cargo horizon-mod` uses (`crates/cargo-horizon-mod/src/sdk_fetch.rs`: `DIR = ".horizon/sdk"`). `.horizon/` is git-ignored |

How the examples get the SDK: `horizon-mod-api = { path = ".horizon/sdk/api/crates/horizon-mod-api" }`,
"fetched into .horizon/sdk/ by any `cargo horizon-mod` command". The command is built from horizon
(`cargo install --locked --git https://github.com/horizon-rust/horizon --rev <rev> cargo-horizon-mod`). It
isn't installed here. Its fetch was reproduced by hand from the signed release assets.

### Package shape (from horizon's docs and template at `2b77287`)

- Flat layout: `horizon-mod.toml`, `Cargo.toml`, `Cargo.lock` (only once there are registry deps,
  `horizon-launcher native-lock .`), `src/lib.rs`, plus optional `assets/<id>/`, `data/<id>/`, `icon.png`,
  and root `README*`/`LICENSE*`/`NOTICE*`. **Assets are allowed in a native package**, so the GUI sprites
  can move from `content/jei/assets/` into the same `.hmod`.
- The `cargo horizon-mod new` native template declares **no `crate-type` and no `[workspace]`**. The launcher
  stages the crate and adds the entry module itself. `crate-type = ["cdylib", "rlib"]` and an empty
  `[workspace]` are harmless to add for local `cargo test`, but the template doesn't require them.
- Refused: a mod `build.rs`, Cargo `[features]`, `#![forbid(unsafe_code)]`, git/path deps outside the mod,
  `rust-toolchain.toml`, a hand-written `horizon-mixin.toml`.
- `[native]`: `game`, `horizon`, `mixins`, optional `[[native.module]]` / `[[native.mixin]]` /
  `[[native.loop]]` / `[[native.bridge]]`. Top-level `internals = [{ path, hash?, kind? }]` declares
  game-internal paths (`cargo horizon-mod internals add|sync`, `native-build --sync-internals`). A mod
  with any `[native.*]` table or internals is **made for exactly one build**. One without them gets a
  `uses.json` and can be checked against later builds (`cargo horizon-mod fits`).
- `id` is `[a-z0-9_]{1,64}` and is also the resource namespace, the `config_files()` owner and the owner
  of settings and keys.

### Hook contract differences that shape the rewrite

- Hooks get `&dyn ClientHost` (screen, render, tooltip, mouse, key) or `&mut dyn ClientHost` (tick, panel,
  command, setting). `give`, `panel_items`, `place_recipe` and `chat_line` need `&mut`. So clicks
  queue an action and `client_tick_post` runs it. `glue.rs` already works this way (`flush`, "what a
  click on a bookmark left for it").
- State lives in the `Hooks` struct or in statics. There is no `&mut Game`.
- Cancellable hooks return `runtime::Flow` instead of setting `*canceled`.

## Hook map (`mixins/jei/horizon-mixin.toml`)

| Old injection | Old target | SDK equivalent | Status |
|---|---|---|---|
| `frame` | `hook = "frame"` (`&mut Game`) | `Hooks::client_tick_post` (`&mut dyn ClientHost`), or `render_frame_pre` | ports |
| `list` | `Game::native_mods` (mod row + keys) | the row comes from `horizon-mod.toml`; keys via `Registrar::register_key_mapping(KeyRegistration)` in `Hooks::register`, read with `ClientInputClientHost::mod_key` | ports |
| `settings` | `Game::native_settings` (free-form button labels) | `Registrar::register_setting(SettingSpec)`: `SettingKind::Toggle` / `Choice` / `Range` / `Color` with translation-key labels | ports, **different UI**: declarative rows, not arbitrary buttons |
| `click` | `Game::native_settings_click` | `Hooks::setting_changed(mod_id, key)` + `ConfigHost::setting` / `setting_bool` / `setting_choice` | ports (note: `set_setting` doesn't fire `setting_changed`) |
| `command` | `Game::native_command` | `Hooks::register_client_commands` (push `Command` usages: `jei`, `jei give`, `jei sort <order:word>`, `jei bookmarks clear`, `jei config`) + `Hooks::command` → `Flow` | ports |
| `event` | `Game::native_panel_event` | `Hooks::panel_event(mod_id, &PanelEvent)`: `Clicked{index,right}`, `Search(String)`, `Key{key,stack}`, `SlotClicked{recipe,slot,stack,right}`, `Action` | ports |
| `screen_post` | `screen_render_post` (`HudBatch`, `NativeScreenView`) | `Hooks::screen_render_post(&dyn ClientHost, ScreenRenderPostEvent{draw: &mut dyn DrawContext, screen: &ScreenView})` | ports |
| `gui_post` | `gui_render_post` | `Hooks::gui_render_post(GuiRenderPostEvent{batch: &mut dyn DrawContext, gui_hidden, focused})` | ports |
| `mouse_press` | `screen_mouse_button_pressed_pre` | same name, returns `Flow` | ports |
| `mouse_release` | `screen_mouse_button_released_pre` | same name | ports |
| `mouse_drag` | `screen_mouse_dragged_pre` | same name | ports |
| `mouse_scroll` | `screen_mouse_scrolled_pre` | same name | ports |
| `key_press` | `screen_key_pressed_pre` | same name | ports |
| `screen_closing` | `screen_closing` | same name | ports |
| `tooltip` | `tooltip` (`&ItemStack`, `&mut Vec<(Text,u32)>`) | `Hooks::item_tooltip` (listen; `stack`, `lines: &mut Vec<(Text,u32)>`, `advanced`, `creative`) | ports |
| `datapack_sync` | `Game::native_datapack_sync` (client side, `recipe_types`) | `Hooks::datapack_sync` exists but is **server-side**: the mod must be `side = "both"` and installed on the server (single player runs it). There's no client-side way to list the server's recipe types. `RecipesHost::recipe_types` is horizon-rust/horizon#19208, still open | partial: needs `side = "both"`, or hard-code `SYNCED_RECIPE_TYPES` |
| `recipes_received` | `Game::native_recipes_received` | `Hooks::recipes_received(RecipesReceivedEvent{recipe_types})` | ports |
| *(new, useful)* | | `Hooks::screen_room` (reserve room right of the container), `screen_exclusion_areas`, `ScreensClientHost::screen_exclusion` | available |

## `crate::game::Game` methods used by `glue.rs` / `kit_layer.rs`

| Old call | SDK equivalent | Status |
|---|---|---|
| `native_on(ID)` | `ConfigHost::on(mod_id)` | ports |
| `native_setting` / `native_set_setting` | `ConfigHost::setting` / `set_setting` (registered keys only) | ports |
| `native_chat(&str)` | `NetworkClientHost::chat_line(Text)` | ports |
| `native_mod_key(id, key)` | `ClientInputClientHost::mod_key(mod_id, key)` / `is_down` | ports |
| `native_fill` | `DrawContext::fill(rect, argb)` | ports |
| `native_quad` | `DrawContext::quad` | ports |
| `native_text` / `native_text_width` | `DrawContext::text` / `text_width` | ports |
| `native_sprite` | `DrawContext::sprite(name, src, dst, tint)` (content GUI texture `<id>:gui/...`) | ports |
| `native_sprite_size` | `ClientRenderClientHost::gui_sprite(&Identifier) -> Option<AtlasRegion>` | ports |
| `native_gui_draw_item_with` | `DrawContext::gui_draw_item_with(stack, at, text)` | ports |
| `native_gui_tooltip` | `DrawContext::gui_tooltip(&[Text], at)` | ports |
| `native_item_stack` | `ItemsHost::item_from_snbt`; `DrawContext::item` / `item_stack` | ports |
| `native_give` | `ItemsClientHost::give(ItemKey, count)` (creative only, one stack) | ports (from a deferred tick, needs `&mut`) |
| `native_transfer_recipe` | `RecipesClientHost::transfer_recipe(book_id)` / `transfer_recipe_check` / `place_recipe(book_id, all)`; ids from `recipe_book_ids(recipe_id)` and `known_recipes` | ports |
| `native_recipes()` (the whole catalog) | `RecipesHost::recipes_of_type(kind)` per type → `Recipe{id, kind, group, ingredients, results, rule, json…}` | ports, per type. Shaped grids must be rebuilt from `json`'s pattern (empty cells are left out), as the Recipes port does |
| `native_panel_items(ID, stacks)` | `RecipesClientHost::panel_items(mod_id, Vec<Stack>)` (≤ 8192) + `panel_style(mod_id, PanelStyle::Classic{centred_search, window_width})`, which the docs describe as JEI's 1.12 look | ports |
| `native_panel_recipes(…)` | **missing** from the public API. Proposed as `RecipesClientHost::panel_recipes` in horizon-rust/horizon#19392 (open); the Recipes WASM port (horizon-rust/horizon-mods#1582) is a draft waiting on it. Still in `internals.json` as `horizon_client::game::native_hooks::Game<'_>::native_panel_recipes` | missing, **or** internals declaration; **or** draw our own recipe window (which `kit_window.rs` already does) |
| `native_player()` | `MenusClientHost::local_player()` / `PlayerRef::is_creative` | ports |
| `native_mod_screen_open` / `_close` | `ScreensClientHost::mod_screen_open` / `mod_screen_close` / `mod_screen` | ports |
| `game.cursor` | `ClientInputClientHost::cursor()` | ports |
| `game.screen` (`Screen::Inventory/Creative/Playing/Win`) | `ScreenView.id` (`"inventory"`, `"creative"`, …); no screen = playing | ports |
| `game.gui_size()` | `DrawContext::size()` | ports |
| `game.translate` / `horizon_client::lang::get_or` | `ConfigClientHost::translate(key)` | ports |
| `crate::game::native_recipes::components(stack)` | `Stack.components` (JSON form) / `ItemStack::to_nbt` handle | ports |
| `horizon_client::item_data::{command_components, decode_components}` | `ItemsHost::item_from_snbt`; `Stack.components` is already the game's JSON form | ports (not in `internals.json`) |
| `horizon_client::containers::{mod_menu_type(s)}` | `ScreenView.menu`; `KeysHost::menu_keys()` | ports |
| `horizon_client::text::Text::{literal, legacy}` | `mod_api::Text::literal`, `TextHost::text_from_legacy` | ports |
| `horizon_client::hud::HudBatch` | `&mut dyn DrawContext` | ports |

## `kit_*.rs` (the "JEI kit / mod API": recipe window, collection tabs, creative)

| Old internal | SDK equivalent | Status |
|---|---|---|
| `game.gui.atlas`, `HudAtlas`, `MISSING_SPRITE` | `DrawContext::sprite` returns `false` for a missing sprite; `gui_sprite` for sizes | ports |
| `horizon_client::ui::nine_slice` | `DrawContext::gui_nine_slice` | ports |
| `horizon_client::widgets::{record, record_as, Kind::Button/Tab}` (narration) | `DrawContext::widget_button` records itself; no generic `record_as` | partial: buttons yes, custom tab/slot narration missing |
| `game.draw_item` | `DrawContext::item_stack` / `gui_draw_item_with` | ports |
| `game.stack_tooltip` + `draw_tooltip_lines` | `DrawContext::gui_tooltip(&[Text])`; the stack's own vanilla lines aren't exposed (`ItemsClientHost::hover_name` gives the name only) | partial (internals: `horizon_client::game::stations::Game<'_>::stack_tooltip`, `draw_tooltip_lines`) |
| `game.res.items()` (item table) | `KeysHost::item_keys`, `ItemsHost::{max_stack_size, item_default_component, …}`, `ItemsClientHost::hover_name`, `enchantments()`, `KeysHost::registry_entries("minecraft:potion")` etc. for component variants | ports (variants are rebuilt from registries, not read from the game's list) |
| `crate::game::native_panel::browser_stack` | `Stack` → `ItemsHost::item_from_snbt` | ports (internal also exists) |
| `game.mod_browser_typing()` | no public "is the panel's search focused" | missing (internal `horizon_client::game::native_panel::Game<'_>::mod_browser_typing`) |
| `game.pack.names.display_item` | `ItemsClientHost::hover_name` | ports |
| `horizon_client::mobs::{Mob, render::render_with}`, `res.mob_types()`, `res.mob_data()` | `DrawContext::entity(&GuiEntity, GuiEntityView)`; `KeysHost::entity_type_keys` | ports (mob preview) |
| `horizon_client::mobs::render::PROFESSIONS` | `KeysHost::registry_entries("minecraft:villager_profession")` | ports |
| `game.res.stations().trades` (villager trades tab) | client side: none. `MerchantRef::offers` is a server handle | missing (internal `horizon_client::game::WorldTables::stations` exists; `trades` isn't in the catalog by that name) |
| `Items::loot` → `horizon_world::loot::LootTables` (`kit_loot_items.rs`, `kit_loot_keys.rs` add `impl` blocks inside the game) | no client loot-table read. `ResourcesHost::resource(PackType::ServerData, id)` reads one JSON at a time; `loot_table_load` is a data-load hook | missing / **support module** (`[[native.module]]` into `horizon-items` and `horizon-world`, the same splice as today) |
| `horizon_world::nbt::{read_named, Limits::templates, Tag}` (structure templates for the collection tab) | `Tag`/`Compound` types exist; no NBT file reader. `resource()` gives the bytes | missing (not in `internals.json`). Would need a small NBT reader in `src/` |
| `game.options.resources` | `ConfigHost::instance_dir()` + `resource()` | partial |
| `game.creative_screen.{tab, first_row}`, `game.slot_at`, `game.slot_view` (`kit_creative.rs`: R/U over creative slots) | `ScreensClientHost::screen_hovered_item()` (item key under the pointer on a container screen); `PanelEvent::Key{key, stack}` already fires over the screen's own slots | ports for R/U. The rest is internals (`horizon_client::game::Game<'_>::creative_screen`, `inventory_slots::Game<'_>::slot_at/slot_view`) |
| `horizon_client::creative::Creative` search text | none | missing |
| `crate::game::native_panel::hz_kit_panel_jei::{has_recipes, listed_at, changed_items, items_version}` (`kit_panel.rs`, a module spliced into `native_panel.rs` for *other* mods) | the mod keeps its own list, so `listed_at` and friends become plain functions on our state. Cross-mod access needs mod dependencies | ports internally; the "JEI kit for other mods" API is out of MVP scope |
| `jei_optional` proc-macro crate (`[[crate]]`) | no `[[crate]]` route; a proc-macro would be a registry dep or a local crate | drop: it only gated the kit on the game having those modules |

## Pure code (moves to `src/` unchanged or nearly)

- `runtime.rs`: search grammar (`tokenize`, `Token`, `Search`), `Sort`, `Viewer`, `View`, `Layout`, `Bookmarks` +
  `bookmarks_parse`, `Config::parse`, `Lang`, `command`, `tooltip_lines`, `overlay_screen`, `screen_grid`,
  `bookmark_bar`, `fits`, `wrap`, `count_range`, `harvest_variants`. No `crate::game` uses (it's already the root
  crate's lib and drives `tests/daily_use.rs`).
- `kit_math.rs`: pure (`tests/kit_math.rs`).
- `kit_collection.rs`: mixed. Its data model is pure; its loaders read loot tables, structures, trades and mobs
  (see above).
- `config/jei/` persistence → `ConfigHost::config_files()` (`ConfigFiles::read/write/list/stat` on a
  `ConfigPath`, scoped to the mod's **own id**: see "the id" below).

## Feasibility

**MVP items 1–4 port cleanly onto the public API**, with no internals and no `[native.*]` tables:

1. Item list beside container screens with search: `panel_items` + `PanelStyle::Classic` + `panel_event`
   (`Search`, `Clicked`), `screen_room`/`screen_exclusion` if we lay out more.
2. R/U: `register_key_mapping` + `PanelEvent::Key{key, stack}` (fires over the panel *and* the screen's own
   slots) or `screen_hovered_item`; the recipe/uses page is drawn by us (`gui_render_post` / `mod_screen_open`),
   from `recipes_of_type`.
3. Tooltip hints: `item_tooltip`.
4. Bookmarks bar + persistence: `screen_render_post` drawing, `screen_mouse_*_pre`, `config_files()`.

Item 5 is mostly available: recipe screen drawn by us, "+" (`recipe_book_ids` + `place_recipe` /
`transfer_recipe`), creative give (`give`), settings (`register_setting`), `/jei` (`register_client_commands` +
`command`). The gaps:

- recipe pages *in the game's panel* (`panel_recipes`, horizon#19392 open). Not needed if we keep JEI's own
  window.
- the server's recipe types from a client-only mod (horizon#19208 open, or `side = "both"` + `datapack_sync`).
- the "kit" extras: loot-table drops, structure/collection tabs, villager trades, the creative screen's search
  text, panel-search focus, full vanilla tooltip lines. These need internals declarations or `[[native.module]]`
  splices, which pin the mod to exactly one Horizon build, or they wait for SDK additions (`mod API:` issues on
  horizon-rust/horizon).

Every public API method the MVP needs also reports `wasm = true` in `surface.json`, so the same code could
ship as a WebAssembly `.hmod`. That's how zoom, keystrokes, coords-hud, fps-overlay and the Recipes port
(horizon-mods#1582) were done.

## What the port found (step 2, 0.4.0)

Built and checked in game on launcher 0.2.102 / Horizon `2b77287` (instance "JEI Testing").

### Gaps beyond the ones above

- **Stack subtypes.** `Stack.components` and `ItemStack` components come as the game's JSON form, while
  `runtime`'s subtypes and the bookmarks file use the `[k=v]` command form. The port lists and bookmarks
  item ids only. Potions and tipped arrows are not separate entries yet.
- **Brewing.** No recipe type or SDK call lists potion mixes, so the brewing tab is empty.
- **Creative screen.** The game shows no mod panel on the creative inventory. The list appears on the
  survival inventory and on container and station screens.
- **"+" and the recipe book.** `recipe_book_ids` answers only for recipes the player has unlocked. The
  port resolves the book id when "+" is pressed and says so in chat when the recipe is locked.
- **Docked view.** `panel_recipes` is still missing (horizon-rust/horizon#19392). `Config.docked` is
  parsed but ignored, and JEI's own recipe screen always shows the pages.

### Packaging rules `native-build` enforces

- Rust files only under `src/`: `unexpected package path: tests/daily_use.rs` (also `mixins/…`). The
  tests moved to `src/tests/`.
- The SDK's crates keep their identity. A registry `serde_json` is a different crate from
  `mod_api::serde_json`, and the build fails with type mismatches. Use the SDK's re-export.
- `Cargo.lock` must keep the game's pins (`game or pinned package proc-macro2 moved…`).
  `horizon-launcher native-lock` resolves the newest versions, so it is wrong for a mod with no
  registry dependencies of its own. Seed the lock from `.horizon/sdk/api/Cargo.lock` instead.
- The manifest `id` must match `[a-z0-9_]{1,64}` (a planned `jei-new` would be refused). The mod keeps `jei`.
