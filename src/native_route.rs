//! What the native route's helpers (`src/mixins/`, compiled into the game, see horizon-mod.toml) ask this crate.
//! The game calls them on its own thread, inside the host call that set them up, so a plain lock is enough.
use std::sync::Mutex;

/// The item and components of the give in flight: `give` (the SDK) carries only an item, so the variant's
/// components wait here for the game's `recipe_give`, which the `give_components` mixin asks.
static GIVE: Mutex<Option<(String, String)>> = Mutex::new(None);

/// Sets (or, with an empty item, clears) the components the next give of `item` carries.
pub(crate) fn offer_give(item: &str, components: &str) {
    let mut give = GIVE.lock().unwrap_or_else(|e| e.into_inner());
    *give = (!item.is_empty() && !components.is_empty())
        .then(|| (item.to_owned(), components.to_owned()));
}

/// For the game's `recipe_give` helper: the components to give `item` with, if this mod is giving a variant
/// of it now. Taken once.
pub fn give_components(item: &str) -> Option<String> {
    let mut give = GIVE.lock().unwrap_or_else(|e| e.into_inner());
    match give.take() {
        Some((wanted, components)) if wanted == item => Some(components),
        other => {
            *give = other;
            None
        }
    }
}

/// For the container screens' draw helpers: whether JEI's recipe screen is open (it then replaces the screen).
pub fn recipe_screen_open() -> bool {
    crate::glue::st().overlay.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_give_carries_its_components_once() {
        offer_give(
            "minecraft:potion",
            "[minecraft:potion_contents={\"potion\":\"minecraft:swiftness\"}]",
        );
        assert_eq!(give_components("minecraft:stick"), None);
        assert!(give_components("minecraft:potion").is_some());
        assert_eq!(give_components("minecraft:potion"), None);
        offer_give("minecraft:potion", "");
        assert_eq!(give_components("minecraft:potion"), None);
    }
}
