fn give_components(
    game: &mut Game<'_>,
    item: &str,
    count: u32,
) -> ::core::ops::ControlFlow<Result<(), String>> {
    // `give` (the SDK) carries an item only; a click on a variant in JEI's list (a potion, an enchanted book)
    // leaves its components with the mod, and the game's own creative give takes them here.
    let Some(components) = ::jei::native_route::give_components(item) else {
        return ::core::ops::ControlFlow::Continue(());
    };
    if !game.creative {
        return ::core::ops::ControlFlow::Continue(());
    }
    game.mod_give(item, count.clamp(1, 99), &components);
    ::core::ops::ControlFlow::Break(Ok(()))
}
