fn recipe_screen(game: &mut Game<'_>, batch: &mut HudBatch) -> ::core::ops::ControlFlow<()> {
    // JEI's recipe screen replaces the open screen, as JEI's RecipesGui does: while it is open the
    // container is not drawn (its menu stays open, so "+" can still fill its grid), only the vanilla
    // background and the item list beside the recipe window. The window itself draws in gui_render_post.
    if !::jei::native_route::recipe_screen_open() {
        return ::core::ops::ControlFlow::Continue(());
    }
    game.render_transparent_background("inventory", batch);
    if let Some(tooltip) = game.draw_mod_browser(batch) {
        game.draw_mod_browser_tooltip(batch, tooltip);
    }
    ::core::ops::ControlFlow::Break(())
}
