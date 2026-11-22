# Handover: the kit extras as their own Horizon mod

JEI for Horizon 0.3 shipped a "JEI kit". It added pages that real JEI doesn't have, closer to
*Just Enough Resources* (JER):

- **Loot drops**: what blocks, mobs, chests, fishing and archaeology drop, with chances.
- **Ore generation**: which heights and biomes an ore generates at, and how much.
- **Villager trades**: what each profession and level sells and buys.
- **Structure / collection tabs**: what a structure is built from.

The SDK port of JEI (0.4, mod id `jei`) leaves them out on purpose:

- They are not JEI features.
- Their data exists only on the server side.
- The 0.3 code (about 4–5k lines) was written against game internals that have since moved.

This document hands them over to a **separate mod**, a fork of this repository trimmed to those pages.

## Where the 0.3 code is

All of it is in git history on `main` (before `sdk-port`), under `mixins/jei/`:

| File | What it holds |
|---|---|
| `kit_collection.rs` (2018 lines) | Loot-table walk with exact chances, worldgen ore heights and veins, structure palettes, mob spawns and villager trades, built into an `Index` on a background thread. The cards for each item. |
| `kit_window.rs` (1736 lines) | The kit's own recipe window: tabs, "cards" (text, stacks, chance bars), mob previews. |
| `kit_layer.rs` (811 lines) | The registration API other mods used (`hz_kit_layer_jei::register`), plus provider sources and notes. |
| `kit_math.rs` (197 lines) | Number providers, height distributions, vein sizes and chance formatting. It is pure, and already ported to `src/kit_math.rs` with tests. |
| `kit_creative.rs`, `kit_panel.rs`, `kit_loot_items.rs`, `kit_loot_keys.rs`, `kit_marker.rs` | Creative-screen R/U, panel helpers for other mods, and `impl` blocks spliced into `horizon-items` and `horizon-world` for loot access. |

`src/kit_collection.rs` in the SDK port keeps only the pure loot-condition evaluator (`condition`, `conditions`,
`count_of`), with tests.

## What the SDK gives a new mod (Horizon `2b77287`)

| Need | SDK | Notes |
|---|---|---|
| Loot tables, worldgen and structure files | `ResourcesHost::resource(PackType::ServerData, id)` | Server side only: it returns `None` on a client. |
| The list of loot tables | `KeysHost::registry_entries("minecraft:loot_table")`, if the registry is exposed; otherwise walk known ids | Check in game. |
| Structure blocks | `world_edit::structure_template(id, …)` | Server side. |
| Villager trades | `MerchantRef::merchant_offers` | Only on a live merchant. Trades in 26.3 may be data-driven: check `data/minecraft/villager_trade/` or similar in the game's data. |
| Mob previews | `DrawContext::entity(&GuiEntity, GuiEntityView)`, `KeysHost::entity_type_keys` | Client. |
| Drawing pages | `DrawContext` (`fill`, `sprite`, `gui_nine_slice`, `item_stack`, `gui_tooltip`) | Client. |

Because the data is server-side, the new mod has two routes:

1. **`side = "both"`**: read everything in a server hook such as `server_started` or `datapack_sync`, which run
   on the integrated server in single player. Then hand the result to the client half through a shared
   static, or through a payload for a dedicated server that runs the mod too. This works in single player
   and on a LAN host. On a dedicated server it works only if the server also runs the mod.
2. **The native route**: `[[native.mixin]]` / `[[native.module]]` on the game's own loot and station tables,
   as 0.3 did. It pins the mod to one Horizon build. `docs/port-sdk.md` lists the internal paths:
   - `horizon_client::game::WorldTables::stations`
   - loot: `horizon_world::loot::LootTables`, which needs a `#[replaceable]` marker or a game-crate caller

## How it would hook into JEI

There is no cross-mod API between the two yet. Options:

- **Standalone**: the new mod draws its own window, opened by its own key (as JER's is) or by a button.
- **Through JEI**: JEI exposes a small public API (categories with chance outputs). `runtime::Category` /
  `ChanceRecipe` / `Layout::Chance` already exist and are tested, but nothing feeds them. The new mod would
  declare JEI as a `[dependencies]` entry and call it. Horizon's physical linking between native mods is
  described in the modding book, under "Dependencies between mods".

## Suggested first steps

1. Fork this repository to a new id, for example `jer_horizon`, and keep `tools/fetch-sdk.sh`, CI and the pinned SDK.
2. Port `kit_math.rs` (done) and the loot evaluator (done). Then write a loader that reads loot tables with
   `resource(PackType::ServerData, …)` in a server hook, for the `side = "both"` route.
3. Prove one page in game, such as "Diamond Ore drops", before porting the window and the cards.
4. Decide on JEI integration (standalone or API) with JEI's maintainer before writing `kit_layer`'s replacement.
