//! JEI for Horizon: an item and recipe viewer, a port of Just Enough Items (mezz, MIT), as a native Horizon mod.
//!
//! `runtime` is the viewer (search, sort, recipe pages, bookmarks, config), with no game in it; `glue` wires it
//! to the mod API; `window` draws JEI's recipe screen; `settings` is the mod's page and `/jei`.
pub mod api;
pub mod kit_collection;
pub mod kit_math;
pub mod runtime;

mod glue;
#[doc(hidden)]
pub mod native_route;
mod settings;
mod variants;
mod window;

#[cfg(test)]
mod tests;

pub use api::{clear_category, register_category, set_chance_recipes};
pub use glue::{shaped, tags_named};
pub use runtime::*;
pub use settings::{COMMANDS, settings};

use mod_api::{
    ClientHost, ClientTickPostEvent, CommandEvent, Flow, GuiRenderPostEvent, Hooks,
    ItemTooltipEvent, KeyRegistration, PanelEventEvent, RecipesReceivedEvent,
    RegisterClientCommandsEvent, Registrar, RegistrationContext, ScreenClosingEvent,
    ScreenKeyPressedPreEvent, ScreenMouseButtonPressedPreEvent, ScreenMouseButtonReleasedPreEvent,
    ScreenMouseDraggedPreEvent, ScreenMouseScrolledPreEvent, ScreenRenderPostEvent,
    SettingChangedEvent,
};

/// The mod's keys: id, label (a translation key) and default (the Java mod's R and U, and A for a bookmark).
pub const KEY_MAPPINGS: [(&str, &str, &str); 3] = [
    ("recipe", "jei.key.recipe", "key.keyboard.r"),
    ("uses", "jei.key.uses", "key.keyboard.u"),
    ("bookmark", "jei.key.bookmark", BOOKMARK_KEY),
];

fn flow(cancel: bool) -> Flow {
    if cancel { Flow::Cancel } else { Flow::Continue }
}

#[derive(Default)]
struct Jei;

#[mod_api::horizon_mod]
impl Hooks for Jei {
    fn register(&self, r: &mut dyn Registrar, context: &RegistrationContext) {
        // The installed mods' names, for the list's mod order and the tooltips' mod line.
        let mut names = std::collections::HashMap::new();
        for package in context.mods().iter() {
            if package.name().trim().is_empty() {
                continue;
            }
            for namespace in package.namespaces() {
                names
                    .entry(namespace.to_owned())
                    .or_insert_with(|| package.name().to_owned());
            }
        }
        native_route::set_mod_names(names);
        for setting in settings() {
            // Only a malformed spec is refused, and the tests check every spec.
            let _ = r.register_setting(setting);
        }
        for (id, label, key) in KEY_MAPPINGS {
            r.register_key_mapping(KeyRegistration::new(id, key).label(label));
        }
    }

    /// Builds the viewer once a world is open, and runs what the input hooks left.
    fn client_tick_post(&self, host: &mut dyn ClientHost, _: &mut ClientTickPostEvent) {
        glue::sync(host);
        glue::flush(host);
    }

    fn recipes_received(&self, _: &mut dyn ClientHost, _: &mut RecipesReceivedEvent<'_>) {
        glue::recipes_received();
    }

    fn panel_event(&self, host: &mut dyn ClientHost, event: &mut PanelEventEvent<'_>) {
        if event.mod_id == ID {
            glue::panel_event(host, event.event);
        }
    }

    /// Every item tooltip ends with the mod the item comes from, in blue, as JEI's do.
    fn item_tooltip(&self, host: &dyn ClientHost, event: &mut ItemTooltipEvent<'_>) {
        glue::item_tooltip(host, event.stack, event.lines);
    }

    fn screen_render_post(&self, host: &dyn ClientHost, event: &mut ScreenRenderPostEvent<'_>) {
        glue::screen_render_post(host, &mut *event.draw, event.screen);
    }

    /// The recipe screen draws after the whole GUI, so a mod's own screen never covers it.
    fn gui_render_post(&self, host: &dyn ClientHost, event: &mut GuiRenderPostEvent<'_>) {
        window::gui_render_post(host, &mut *event.batch);
    }

    fn screen_mouse_button_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonPressedPreEvent<'_>,
    ) -> Flow {
        flow(glue::mouse_pressed(
            host,
            event.screen,
            event.button,
            event.modifiers,
        ))
    }

    fn screen_mouse_button_released_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseButtonReleasedPreEvent<'_>,
    ) -> Flow {
        flow(glue::mouse_released(host, event.screen, event.button))
    }

    fn screen_mouse_dragged_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseDraggedPreEvent<'_>,
    ) -> Flow {
        flow(glue::mouse_dragged(host, event.screen, event.button))
    }

    fn screen_mouse_scrolled_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenMouseScrolledPreEvent<'_>,
    ) -> Flow {
        flow(glue::mouse_scrolled(host, event.screen, event.scroll))
    }

    fn screen_key_pressed_pre(
        &self,
        host: &dyn ClientHost,
        event: &mut ScreenKeyPressedPreEvent<'_>,
    ) -> Flow {
        flow(glue::key_pressed(host, event.screen, event.key))
    }

    fn screen_closing(&self, host: &dyn ClientHost, _: &mut ScreenClosingEvent<'_>) {
        glue::screen_closing(host);
    }

    fn register_client_commands(
        &self,
        _: &mut dyn ClientHost,
        event: &mut RegisterClientCommandsEvent<'_>,
    ) {
        event.commands.extend(settings::commands());
    }

    fn command(&self, host: &mut dyn ClientHost, event: &mut CommandEvent<'_>) -> Flow {
        flow(settings::command(host, event.command))
    }

    fn setting_changed(&self, host: &mut dyn ClientHost, event: &mut SettingChangedEvent<'_>) {
        if event.mod_id == ID {
            settings::changed(host, event.key);
        }
    }
}
