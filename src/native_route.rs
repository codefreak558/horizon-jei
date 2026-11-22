//! What the native route's helpers (`src/mixins/`, compiled into the game, see horizon-mod.toml) ask this crate.
//! The game calls them on its own thread, inside the host call that set them up, so a plain lock is enough.
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

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

/// The item list's area as the game drew it last, and the pages the wheel asked for since: the game pages the
/// list but reads no wheel there, so `screen_mouse_scrolled_pre` leaves the steps here and the
/// `panel_scroll` mixin (on the list's draw) turns the page.
struct PanelScroll {
    area: Option<([f32; 4], Instant)>,
    pending: i32,
}

static PANEL: Mutex<PanelScroll> = Mutex::new(PanelScroll {
    area: None,
    pending: 0,
});

/// How long the list's last drawn area counts: a few frames, so a list no longer drawn takes no wheel.
const PANEL_AREA_FRESH: Duration = Duration::from_millis(250);

/// For the item list's draw helper: records where the list is and takes the pages to turn (forward positive).
pub fn panel_frame(area: [f32; 4]) -> i32 {
    let mut panel = PANEL.lock().unwrap_or_else(|e| e.into_inner());
    panel.area = Some((area, Instant::now()));
    std::mem::take(&mut panel.pending)
}

/// The wheel turned by `dy` with the pointer at `mouse`: true when it is over the item list, which then turns
/// a page at its next draw. Down is the next page, as on the recipe screen.
pub(crate) fn panel_scroll(mouse: [f32; 2], dy: f64) -> bool {
    let mut panel = PANEL.lock().unwrap_or_else(|e| e.into_inner());
    let Some(([x, y, w, h], at)) = panel.area else {
        return false;
    };
    let over = mouse[0] >= x && mouse[1] >= y && mouse[0] < x + w && mouse[1] < y + h;
    if dy == 0.0 || !over || at.elapsed() > PANEL_AREA_FRESH {
        return false;
    }
    let step = if dy < 0.0 { 1 } else { -1 };
    panel.pending = panel.pending.saturating_add(step);
    true
}

/// The installed mods' display names, by each namespace they declare (read once at registration).
static MOD_NAMES: Mutex<Option<HashMap<String, String>>> = Mutex::new(None);

pub(crate) fn set_mod_names(names: HashMap<String, String>) {
    *MOD_NAMES.lock().unwrap_or_else(|e| e.into_inner()) = Some(names);
}

pub(crate) fn mod_names() -> HashMap<String, String> {
    (MOD_NAMES.lock().unwrap_or_else(|e| e.into_inner()))
        .clone()
        .unwrap_or_default()
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

    #[test]
    fn the_wheel_over_the_list_turns_its_pages_at_the_next_draw() {
        // No list drawn yet: the wheel is the screen's.
        assert!(!panel_scroll([5.0, 5.0], -1.0));
        assert_eq!(panel_frame([0.0, 0.0, 100.0, 200.0]), 0);
        assert!(panel_scroll([50.0, 50.0], -1.0));
        assert!(panel_scroll([50.0, 50.0], -1.0));
        assert!(panel_scroll([50.0, 50.0], 1.0));
        // Outside the list, or no turn: not the list's.
        assert!(!panel_scroll([150.0, 50.0], -1.0));
        assert!(!panel_scroll([50.0, 50.0], 0.0));
        assert_eq!(panel_frame([0.0, 0.0, 100.0, 200.0]), 1);
        assert_eq!(panel_frame([0.0, 0.0, 100.0, 200.0]), 0);
    }
}
