fn creative_panel(owner: &str) -> &str {
    // The game shows a native panel on the creative screen only for its own Recipes mod; JEI's list belongs
    // there too, so JEI reads as Recipes to that one check (native_panel.rs, `browser_sync`).
    if owner == "jei" { "recipes" } else { owner }
}
