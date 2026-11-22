//! The mod's page in Options > Mods, and `/jei`. The page's toggles mirror `config/jei/jei.cfg`: a change
//! on the page is written to the file, and the file's values are shown on the page when it is read.
use mod_api::{ClientHost, Command, SettingSpec, Text};

use crate::glue;
use crate::runtime::{self as jei, Config, Sort};

/// The settings on JEI's page in Options > Mods.
pub fn settings() -> [SettingSpec; 5] {
    [
        SettingSpec::toggle("give", "jei.setting.give", false).tooltip("jei.setting.give.tooltip"),
        SettingSpec::toggle("hints", "jei.setting.hints", true)
            .tooltip("jei.setting.hints.tooltip"),
        SettingSpec::toggle("bookmarks", "jei.setting.bookmarks", true)
            .tooltip("jei.setting.bookmarks.tooltip"),
        SettingSpec::choice(
            "sort",
            "jei.setting.sort",
            ["jei.setting.sort.name", "jei.setting.sort.id"],
            0,
        ),
        SettingSpec::choice(
            "language",
            "jei.setting.language",
            ["jei.setting.language.en", "jei.setting.language.es"],
            0,
        ),
    ]
}

/// The client commands, as the chat box completes them.
pub const COMMANDS: [&str; 6] = [
    "jei",
    "jei give",
    "jei sort name",
    "jei sort id",
    "jei bookmarks clear",
    "jei config",
];

fn flag(on: bool) -> &'static str {
    if on { "true" } else { "false" }
}

/// Shows the file's config on the settings page.
pub(crate) fn show(host: &dyn ClientHost, config: Config) {
    host.set_setting(jei::ID, "hints", flag(config.hints));
    host.set_setting(jei::ID, "bookmarks", flag(config.bookmarks));
    let sort = match config.sort {
        Sort::Name => "0",
        Sort::Id => "1",
    };
    host.set_setting(jei::ID, "sort", sort);
}

/// A setting changed on the page: the file follows, and a new order shows at once.
pub(crate) fn changed(host: &mut dyn ClientHost, key: &str) {
    let mut config = glue::config(host);
    match key {
        "hints" => config.hints = host.setting_bool("hints").unwrap_or(true),
        "bookmarks" => config.bookmarks = host.setting_bool("bookmarks").unwrap_or(true),
        "sort" => {
            let sort = match host.setting_choice("sort") {
                Some(1) => Sort::Id,
                _ => Sort::Name,
            };
            glue::set_sort(host, sort);
            return;
        }
        _ => return,
    }
    glue::store_config(host, config);
}

/// `/jei …`: true when it was this mod's.
pub(crate) fn command(host: &mut dyn ClientHost, line: &str) -> bool {
    let line = line.trim().trim_start_matches('/');
    let args = match line.strip_prefix("jei") {
        Some(rest) if rest.is_empty() || rest.starts_with(' ') => rest,
        _ => return false,
    };
    if !host.on(jei::ID) {
        return false;
    }
    let lang = glue::lang(host);
    let text = match jei::command(args) {
        Some(jei::Command::About) => match glue::counts() {
            Some((items, recipes)) => jei::about_counts(items, recipes),
            None => jei::about(),
        },
        Some(jei::Command::ToggleGive) => {
            let on = !host.setting_bool("give").unwrap_or(false);
            host.set_setting(jei::ID, "give", flag(on));
            lang.give_button(on)
        }
        Some(jei::Command::Sort(sort)) => {
            // `set_setting` fires no `setting_changed`: the order is applied here.
            host.set_setting(jei::ID, "sort", if sort == Sort::Id { "1" } else { "0" });
            glue::set_sort(host, sort);
            lang.sort_button(sort)
        }
        Some(jei::Command::SortNone) => lang.sort_button(glue::config(host).sort),
        Some(jei::Command::BookmarksClear) => {
            glue::clear_bookmarks(host);
            lang.bookmarks_cleared().to_owned()
        }
        Some(jei::Command::Config) => glue::files(),
        None => jei::COMMAND_USAGE.to_owned(),
    };
    let _ = host.chat_line(Text::literal(text));
    true
}

/// The commands to register.
pub fn commands() -> impl Iterator<Item = Command> {
    COMMANDS.into_iter().map(Command::new)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_setting_is_one_the_game_takes() {
        for setting in settings() {
            assert_eq!(setting.check(), Ok(()), "{}", setting.key);
        }
    }
}
