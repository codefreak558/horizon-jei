# Configuration

JEI keeps two kinds of files: the game's settings for the mod (one file per key) and its own folder in
the game's `config/`, shared by every mod as on Fabric and NeoForge.

## The game's settings, `horizon-mods/.horizon-mod-data/jei/`

Registered with `register_setting` and read with `setting_bool` / `setting_choice`. They change from the
mod's page in Options → Mods → JEI and from the chat commands. The page also shows `jei.cfg`'s `bookmarks`
and `sort`, and writes them back to the file.

| Key | Values | Default | What it does |
|---|---|---|---|
| `give` | `true` / `false` | `false` | "Click gives items": in creative a click on a panel item gives one, a right click a stack. In survival the click opens the recipes instead. `/jei give` toggles it. |
| `language` | English / Español (choice 0 / 1) | English | The mod's own strings. Item names always come from the game in the player's language. |
| `bookmarks`, `sort` | as in `jei.cfg` | | Mirrors of `jei.cfg`'s keys. |

The mod's keys (`recipe`, `uses`, `bookmark`) are the game's `mods.toml`-style bindings, set on the
same page: R, U and A by default.

## The mod's files, `config/jei/`

| File | What it is |
|---|---|
| `jei.cfg` | The mod's own options, `key=value` a line. |
| `bookmarks.txt` | The bar, one bookmark a line. |

Both are made with their defaults when missing, written when a setting changes, and read once when
the mod first needs them (a world opens, or the settings page opens).
`/jei config` prints their paths.

### `jei.cfg`

| Key | Values | Default | What it does |
|---|---|---|---|
| `sort` | `mod` / `name` / `id` | `mod` | The item list's order: by mod as JEI does (Minecraft first, then each mod by its name, the game's registry order inside each), by the game's name (then id, then variant), or by id. The settings page's "Item order" choice and `/jei sort mod\|name\|id` change it, and the open list is rewritten at once. |
| `hints` | `true` / `false` | `true` | Read and written back for 0.3's files; 0.4 shows no tooltip lines, so it does nothing. |
| `bookmarks` | `true` / `false` | `true` | Draws the bookmark bar over container screens. Turning it off keeps the bookmarks and still bookmarks the stack the A key points at. |
| `recipes` | `screen` / `panel` | `screen` | 0.3's choice of the docked view. Read and kept; 0.4 always uses JEI's recipe screen. |
| `recipe_width`, `recipe_height` | numbers, at least 198 and 175 | `198`, `350` | The recipe window's size, written when an edge drag ends. The window is never wider than the screen less 12 nor taller than it less 58. |
| `navigation` | `paged` / `scrolling` | `paged` | Recipes turn pages, or scroll under a scrollbar. The settings page's "Recipe navigation" choice changes it. |

Unknown keys and unknown values are ignored: an option the file names badly keeps its default.

### `bookmarks.txt`

```
# JEI bookmarks: item<TAB>count<TAB>components
minecraft:diamond	1	
minecraft:potion	1	[minecraft:potion_contents={"potion":"minecraft:strength"}]
```

`components` is the command syntax `/give` reads, cut to the subtype components (empty for a plain
stack). The game's SNBT spelling (`{potion:"minecraft:strength"}`) is read as the same bookmark. Two stacks are the same bookmark when their item and their
components match: a potion of strength and a potion of swiftness are two bookmarks.

Lines starting with `#` and blank lines are skipped; a line without a count is one. The file holds at
most 24 bookmarks, the bar's two columns of twelve; `/jei bookmarks clear` empties it.

## Commands

| Command | What it does |
|---|---|
| `/jei` or `/jei about` | The build and how many items and recipes the tables hold. |
| `/jei give` | Toggles "Click gives items". |
| `/jei sort` | Says the list's order. |
| `/jei sort mod` / `/jei sort name` / `/jei sort id` | Sets it. |
| `/jei bookmarks clear` | Empties the bookmark bar. |
| `/jei config` | The paths of `jei.cfg` and `bookmarks.txt`. |