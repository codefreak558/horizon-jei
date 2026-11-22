# Development environment

Checked on 22 Nov 2026 on Windows 11 with launcher 0.2.102 and Horizon Nightly 2026-11-22 (`2b77287`).

## Toolchain

| Tool | Where |
|---|---|
| Rust 1.99.1 | the launcher's own: `%APPDATA%\horizon-launcher\rust\1.99.1-x86_64-pc-windows-gnu\bin` (`horizon-launcher rust install`). Put it first on `PATH` |
| rustfmt, clippy | not shipped with the launcher's toolchain. Get the 1.99.1 components (rustup, or the rust-lang dist tarballs). CI uses `dtolnay/rust-toolchain` with toolchain 1.99.1 |
| SDK | `tools/fetch-sdk.sh`: `api-source.tar.xz` of release `sdk-2b77287…` (horizon-rust/horizon), SHA-256 checked, unpacked to `.horizon/sdk/api/` |
| API surface | `%APPDATA%\horizon-launcher\native\sdk\2b77287…\surface.json`. Use only APIs listed there |

## Commands

```sh
tools/fetch-sdk.sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
horizon-launcher native-build . --instance "JEI Testing" --trust-native-code
```

`tools/dev-check.sh` runs them all in order.

## What the launcher's build refuses

- Rust files outside `src/` (`unexpected package path: tests/…`). The tests therefore live in
  `src/tests/`, and the old `mixins/` folder had to go.
- A registry crate that the SDK also provides. Use `mod_api::serde_json`, not a `serde_json`
  dependency: the launcher builds against the SDK's own crates, and a registry copy is a different
  type.
- A `Cargo.lock` that moves a crate the game pins (`game or pinned package … moved`). Seed the lock from
  `.horizon/sdk/api/Cargo.lock` and let cargo add the mod. `horizon-launcher native-lock` resolves
  the newest versions, which is wrong here while the mod has no registry dependencies of its own.
- `build.rs`, Cargo `[features]`, `#![forbid(unsafe_code)]`, `rust-toolchain.toml`.

## In-game checks

The launcher's MCP server (`horizon-launcher mcp`) starts the game under agent control: `play`,
`game_open_inventory`, `game_click_at`, `game_key`, `game_screenshot`, `game_screen`. A pass covers:

- the list beside the survival inventory and the search box
- a click and R/U opening the recipe screen, Escape going back
- "+" filling the 2x2 grid
- the A key bookmarking, and the bar surviving a relaunch
- `/jei sort id` writing `config/jei_new/jei.cfg`
