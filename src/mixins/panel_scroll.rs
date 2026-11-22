fn panel_scroll(game: &mut Game<'_>, layout: &Layout) {
    // The game pages the item list but reads no wheel there: JEI's wheel hook leaves the pages to turn with
    // the mod (src/native_route.rs), and the list's draw turns them, within its pages.
    let ours = game
        .panel
        .of
        .as_ref()
        .is_some_and(|(owner, _)| owner.native_id() == "jei");
    if !ours {
        return;
    }
    let steps = ::jei::native_route::panel_frame(layout.area);
    if steps == 0 {
        return;
    }
    let last = game.browser_pages(layout).saturating_sub(1);
    let page = game.panel.page.min(last) as i64 + i64::from(steps);
    game.panel.page = page.clamp(0, last as i64) as usize;
}
