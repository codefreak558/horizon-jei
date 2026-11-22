# JEI for Horizon

An item and recipe viewer for [Horizon](https://playhorizon.dev), the Rust
reimplementation of Minecraft: Java Edition 26.3. It is a port of
[JustEnoughItems](https://github.com/mezz/JustEnoughItems) (JEI), built as a
native mod on Horizon's mod SDK (`horizon-mod-api`).

Status: **0.4.0, the SDK port** (mod id `jei_new`). It is made for the Horizon
build of Nightly 2026-11-22 (`2b77287`). It replaces the 0.3 mixin bundle,
which current launchers refuse to load.

- **Item list** beside container screens and the inventory, drawn in JEI's 1.12
  style (`PanelStyle::Classic`), with JEI's search grammar (`@mod`, `#tag`,
  `-exclude`, `a|b`).
- **R / U** over any stack (the list's or a slot's) opens its recipes or uses.
  So does a click in the list: left for recipes, right for uses.
- **Recipe screen**: JEI's own window over the open screen, with a tab per
  station, catalysts, pages and the mouse wheel. Escape or Backspace goes back.
  **"+"** fills the open crafting grid with a recipe from your recipe book
  (Shift fills as many as you can).
- **Tooltip hints**: `R: Recipes (12)`, `U: Uses (3)` and a bookmark mark in
  every item tooltip.
- **Bookmarks**: **A** bookmarks the stack under the pointer. The bar sits at
  the top left of container screens. Left click opens a bookmark's recipes,
  right click its uses, Shift or middle click removes it, and dragging
  reorders. Kept in `config/jei_new/bookmarks.txt`.
- **Creative give**: with "Click gives items" on, a click in the list gives one
  and a right click gives a stack.
- **Settings** in Options → Mods → JEI: give, tooltip hints, the bookmark bar,
  item order and language (English/Spanish). The toggles are mirrored in
  `config/jei_new/jei.cfg`.
- **Commands**: `/jei`, `/jei give`, `/jei sort name|id`,
  `/jei bookmarks clear`, `/jei config`.

Not ported yet (the SDK has no public API for them, see
[`docs/port-sdk.md`](docs/port-sdk.md)):

- recipe pages inside the game's own panel (0.3's "docked" view)
- the collection tabs: loot drops, ore generation and villager trades
- stack subtypes such as potions and tipped arrows (the list shows item ids only)
- the item list on the creative inventory screen
- brewing recipes

## Install

The mod is a native mod. The launcher compiles it against your instance's
Horizon build. Native mods are not sandboxed, so only build code you trust.

```sh
git clone https://github.com/codefreak558/horizon-jei
cd horizon-jei
tools/fetch-sdk.sh
horizon-launcher native-build . --instance "<your instance>" --trust-native-code
```

The instance must play Horizon `2b77287` (Nightly 2026-11-22), the build
pinned in `horizon-mod.toml`'s `[native]` table. Start the instance and open
your inventory.

## Develop

You need the launcher's own Rust toolchain (1.99.1:
`horizon-launcher rust install`) on your `PATH`, plus `rustfmt` and `clippy`
for that version.

```sh
tools/fetch-sdk.sh        # the pinned SDK into .horizon/sdk/ (git-ignored)
cargo test                # the viewer's logic, no game
tools/dev-check.sh        # fmt, clippy -D warnings, tests, then native-build
```

`horizon-launcher native-build .` is the real build. It compiles the crate
against the prebuilt game with the SDK's own crates, so only the SDK's
`serde_json` (`mod_api::serde_json`) is usable, and `Cargo.lock` must keep the
SDK's pins. See [`docs/dev.md`](docs/dev.md).

## Layout

```
horizon-mod.toml   the manifest: id jei_new, [native] pinned to Horizon 2b77287
Cargo.toml         depends only on .horizon/sdk/api/crates/horizon-mod-api
Cargo.lock         seeded from the SDK's lock (the game's pins)
src/
  lib.rs           #[mod_api::horizon_mod] impl Hooks: the hooks, one line each
  glue.rs          the game's items/recipes in; panel, give, "+" and the overlay out
  window.rs        JEI's recipe screen: drawing and hit tests
  settings.rs      the Options > Mods page and /jei
  runtime.rs       the viewer: search, sort, pages, bookmarks, config (pure Rust)
  kit_math.rs      kit arithmetic (pure); kit_collection.rs loot-condition evaluator (pure)
  tests/           daily_use.rs, kit_math.rs (native packages hold Rust only under src/)
assets/jei_new/    JEI 1.12's GUI sprites (MIT) and the en_us / es_es lang files
tools/             fetch-sdk.sh, dev-check.sh
docs/              port-sdk.md (the SDK map), architecture.md, dev.md, ...
```

## License

MIT. See `LICENSE` and `NOTICE` (attribution to JEI, © mezz, and the port's
authors). Not affiliated with mezz, Mojang, Microsoft or the Horizon project.
