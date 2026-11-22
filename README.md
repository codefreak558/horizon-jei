# JEI for Horizon

An item and recipe viewer for [Horizon](https://playhorizon.dev), the Rust
reimplementation of Minecraft: Java Edition 26.3. It is a port of
[JustEnoughItems](https://github.com/mezz/JustEnoughItems) (JEI), built as a
native mod on Horizon's mod SDK (`horizon-mod-api`).

Status: **0.6.0, the SDK port with an add-on API** (mod id `jei`). It is made for the Horizon
build of Nightly 2026-11-22 (`2b77287`). It replaces the 0.3 mixin bundle,
which current launchers refuse to load.

- **Item list** beside container screens, the inventory and the creative
  screen, drawn in JEI's 1.12 style (`PanelStyle::Classic`), with JEI's search
  grammar (`@mod`, `#tag`, `-exclude`, `a|b`).
- **Subtypes**: every potion, splash and lingering potion, tipped arrow,
  enchanted book level, goat horn and ominous bottle is an entry of its own,
  found by its name ("swiftness").
- **R / U** over any stack (the list's or a slot's) opens its recipes or uses.
  So does a click in the list: left for recipes, right for uses.
- **Recipe screen**: JEI's own window over the open screen, laid out as JEI's.
  It has a tab per category (crafting, smelting, blasting, smoking, campfire,
  stonecutting, smithing, brewing), catalysts, a category bar and a page bar
  with arrows, each recipe on JEI's recipe background, and the mouse wheel.
  Escape or Backspace goes back. **"+"** fills the open crafting grid with a
  recipe from your recipe book (Shift fills as many as you can).
- **Bookmarks**: **A** bookmarks the stack under the pointer. The bar sits at
  the top left of container screens. Left click opens a bookmark's recipes,
  right click its uses, Shift or middle click removes it, and dragging
  reorders. Kept in `config/jei/bookmarks.txt`.
- **Creative give**: with "Click gives items" on, a click in the list gives one
  and a right click gives a stack, variants included.
- **Add-on API**: other mods add categories (tabs) with chance outputs, such as
  [JER](https://github.com/codefreak558/horizon-jer)'s mob drops and villager trades. See
  [`docs/addon-api.md`](docs/addon-api.md).
- **Settings** in Options → Mods → JEI: give, the bookmark bar, item order, recipe navigation and
  language (English/Spanish). The bar and the order are mirrored in
  `config/jei/jei.cfg`.
- **Commands**: `/jei`, `/jei give`, `/jei sort mod|name|id`,
  `/jei bookmarks clear`, `/jei config`.

Two features use **the native route** (`[[native.mixin]]` rows in
`horizon-mod.toml`): the list on the creative screen and the creative give of
a variant. Because of them the mod is made for exactly the pinned Horizon
build and must be re-pinned and rebuilt for each new one.

Not in this mod:

- 0.3's "docked" view (recipe pages inside the game's panel). Real JEI doesn't
  have it.
- The kit's loot drops, ore generation and villager trades. They are JER-style
  pages, not JEI. See [`docs/handover-kit-extras.md`](docs/handover-kit-extras.md)
  for porting them as a separate mod.

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

## Releases

Releases of 0.1–0.3 were signed with minisign. The public key is
[`horizon-jei.pub`](horizon-jei.pub) (key id `FC3386F1F51ECF6A`). The private key
is not in the repository.

From 0.4.0 a release is made by the `release` workflow (Actions, Run workflow,
tag `vX.Y.Z` matching `horizon-mod.toml`'s version, or a pushed `v*` tag). It
tests the crate, packs `jei.hmod` with `cargo horizon-mod build`, runs
`cargo horizon-mod check` on it, and publishes a GitHub release with the package,
its SHA-256 and the version's CHANGELOG section. The package is unsigned.

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
horizon-mod.toml   the manifest: id jei, [native] pinned to Horizon 2b77287, [[native.mixin]] rows
Cargo.toml         depends only on .horizon/sdk/api/crates/horizon-mod-api
Cargo.lock         seeded from the SDK's lock (the game's pins)
src/
  lib.rs           #[mod_api::horizon_mod] impl Hooks: the hooks, one line each
  glue.rs          the game's items/recipes in; panel, give, "+" and the overlay out
  window.rs        JEI's recipe screen: drawing and hit tests
  settings.rs      the Options > Mods page and /jei
  variants.rs      subtypes (potions, books, ...) and brewing recipes
  native_route.rs  what the native route's helpers ask the mod
  mixins/          the [[native.mixin]] helpers (compiled into the game, not into the crate)
  runtime.rs       the viewer: search, sort, pages, bookmarks, config (pure Rust)
  kit_math.rs      kit arithmetic (pure); kit_collection.rs loot-condition evaluator (pure)
  tests/           daily_use.rs, kit_math.rs (native packages hold Rust only under src/)
assets/jei/    JEI 1.12's GUI sprites (MIT) and the en_us / es_es lang files
tools/             fetch-sdk.sh, dev-check.sh
docs/              port-sdk.md (the SDK map), architecture.md, dev.md, ...
```

## License

MIT. See `LICENSE` and `NOTICE` (attribution to JEI, © mezz, and the port's
authors). Not affiliated with mezz, Mojang, Microsoft or the Horizon project.
