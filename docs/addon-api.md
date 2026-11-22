# Add-on API (0.5)

A mod can add categories to JEI's recipe screen. Each is a tab after the vanilla stations, whose recipes list
outputs that drop with a chance (loot, trades, ore). It is client side only.

## Declare the dependency

In the dependent mod's `horizon-mod.toml` (not `Cargo.toml`; the launcher adds the path dependency and a Cargo
feature named `jei`):

```toml
[dependencies]
jei = { version = "^0.5", optional = true }
```

Guard every call, so the mod builds and runs without JEI:

```rust
mod_api::if_installed!(host, jei, {
    jei::register_category("jer:mob_drops", "Mob Drops", "minecraft:rotten_flesh", jei::Layout::Chance);
    jei::set_chance_recipes("jer:mob_drops", recipes);
});
```

## Functions (crate root)

| Function | Does |
|---|---|
| `register_category(id, title, icon_item, layout)` | Declares a tab. Idempotent per `id`: a second call updates title, icon and layout and keeps the recipes. |
| `set_chance_recipes(category_id, Vec<ChanceRecipe>)` | Replaces the category's recipes (send the full set). Recipes for an id not yet registered wait, hidden, until it is. At most 20 000 per category. |
| `clear_category(category_id)` | Empties the category and keeps it registered. Call it when the world is left. Unknown ids do nothing. |

Types (all owned, in `jei::`): `ChanceRecipe { inputs: Vec<Vec<Stack>>, outputs: Vec<Chance>, notes: Vec<String> }`,
`Chance { stack, chance: f32, min: u32, max: u32, notes: Vec<String> }`, `Stack { item, count, components }`
(`Stack::item("minecraft:diamond")`, `Chance::new(stack, chance)`), `Layout` (only `Chance` is drawn today).

## Behaviour

- **R** over an item lists the recipes whose outputs contain it. **U** lists those whose inputs contain it.
- Each recipe's `inputs` are slots, each with its alternatives; `notes` show under the recipe.
- A chance output shows its percentage under the slot. Its tooltip shows the chance, the count range (`min`..`max`)
  and the output's notes (for example "Only when killed by a player").
- Safe under any order and from any client hook: calls before or after JEI builds its tables, repeated calls,
  unknown items or ids. JEI reads the registry on its next client tick. An item JEI does not know matches
  nothing; it never panics.
