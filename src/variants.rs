//! Stack subtypes (JEI's subtype interpreters): potions, splash and lingering potions, tipped arrows,
//! enchanted books, goat horns and ominous bottles as entries of their own, and brewing recipes.
//!
//! The viewer and the game's panel keep components in the command form `/give` reads
//! (`[minecraft:potion_contents={"potion":"minecraft:swiftness"}]`; the game reads it with
//! `command_components`, whatever the SDK's doc of `Stack::components` says), and `item_from_snbt` takes an
//! item compound. This mod writes every value as JSON, which is also valid SNBT, so one value gives both
//! forms; the game's own SNBT spelling (`{potion:"…"}`) is read back with `snbt_to_json`.
use mod_api::serde_json::{Map, Value, json};

use crate::runtime::SUBTYPES;

/// One component of a variant: its name (`minecraft:potion_contents`) and value.
pub type Component = (String, Value);

/// Whether a component name (with or without `minecraft:`) is a subtype one.
fn subtype(name: &str) -> bool {
    SUBTYPES.contains(&name.strip_prefix("minecraft:").unwrap_or(name))
}

/// The command form of components: `[name=value,…]`; "" for none.
pub fn command(components: &[Component]) -> String {
    if components.is_empty() {
        return String::new();
    }
    let parts: Vec<String> = (components.iter())
        .map(|(name, value)| format!("{name}={value}"))
        .collect();
    format!("[{}]", parts.join(","))
}

/// The game's JSON form of components; "" for none.
pub fn json_form(components: &[Component]) -> String {
    if components.is_empty() {
        return String::new();
    }
    let map: Map<String, Value> = components.iter().cloned().collect();
    Value::Object(map).to_string()
}

/// The SNBT item compound `item_from_snbt` reads.
pub fn snbt(item: &str, components: &[Component]) -> String {
    match json_form(components) {
        c if c.is_empty() => format!("{{id:\"{item}\",count:1}}"),
        c => format!("{{id:\"{item}\",count:1,components:{c}}}"),
    }
}

/// An SNBT value as JSON: bare keys and words quoted, single quotes made double, number suffixes
/// (`1b`, `5s`, `2.0f`) dropped. None for text this reads no value from.
pub fn snbt_to_json(snbt: &str) -> Option<Value> {
    let chars: Vec<char> = snbt.trim().chars().collect();
    let mut out = String::with_capacity(chars.len() + 8);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '"' | '\'' => {
                out.push('"');
                i += 1;
                while i < chars.len() && chars[i] != c {
                    if chars[i] == '\\' && i + 1 < chars.len() {
                        out.push('\\');
                        out.push(chars[i + 1]);
                        i += 2;
                        continue;
                    }
                    if chars[i] == '"' {
                        out.push('\\');
                    }
                    out.push(chars[i]);
                    i += 1;
                }
                out.push('"');
                i += 1;
            }
            '{' | '}' | '[' | ']' | ',' | ':' => {
                out.push(c);
                i += 1;
            }
            c if c.is_whitespace() => i += 1,
            _ => {
                let start = i;
                while i < chars.len() && !"{}[],:\"' \t\n".contains(chars[i]) {
                    i += 1;
                }
                let word: String = chars[start..i].iter().collect();
                let next = chars[i..].iter().find(|c| !c.is_whitespace());
                let number = word
                    .trim_end_matches(['b', 'B', 's', 'S', 'l', 'L', 'f', 'F', 'd', 'D'])
                    .to_owned();
                if next == Some(&':') {
                    out.push_str(&Value::String(word).to_string());
                } else if word == "true" || word == "false" {
                    out.push_str(&word);
                } else if !number.is_empty() && number.parse::<f64>().is_ok() {
                    out.push_str(&number);
                } else {
                    out.push_str(&Value::String(word).to_string());
                }
            }
        }
    }
    mod_api::serde_json::from_str(&out).ok()
}

/// The subtype components of a command form (`[minecraft:potion_contents={potion:"…"}]`, as the game and this
/// mod write it), sorted by name; the others are left out.
pub fn from_command(command: &str) -> Vec<Component> {
    let inner = command.trim();
    let Some(inner) = inner.strip_prefix('[').and_then(|s| s.strip_suffix(']')) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let (mut depth, mut quote, mut start) = (0i32, false, 0usize);
    let mut parts = Vec::new();
    let mut escaped = false;
    for (i, c) in inner.char_indices() {
        match c {
            _ if escaped => escaped = false,
            '\\' if quote => escaped = true,
            '"' => quote = !quote,
            '{' | '[' if !quote => depth += 1,
            '}' | ']' if !quote => depth -= 1,
            ',' if !quote && depth == 0 => {
                parts.push(&inner[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&inner[start..]);
    for part in parts {
        let Some((name, value)) = part.split_once('=') else {
            continue;
        };
        let name = name.trim();
        if !subtype(name) {
            continue;
        }
        let value = (mod_api::serde_json::from_str::<Value>(value.trim()).ok())
            .or_else(|| snbt_to_json(value));
        if let Some(value) = value {
            let name = if name.contains(':') {
                name.to_owned()
            } else {
                format!("minecraft:{name}")
            };
            out.push((name, value));
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// A command form in this mod's own spelling (JSON values, subtype components only), so the game's
/// `{potion:"…"}` and this mod's `{"potion":"…"}` are the same variant.
pub fn canonical(command: &str) -> String {
    self::command(&from_command(command))
}

/// The variants the game's registries name, item by item, as `(item, components)`: every potion as a potion,
/// splash potion, lingering potion and tipped arrow; every enchantment at every level as an enchanted book;
/// every goat horn sound; the ominous bottle's five levels. `has` says whether the game has an item.
pub fn registry_variants(
    has: &dyn Fn(&str) -> bool,
    potions: &[String],
    enchantments: &[(String, u32)],
    instruments: &[String],
) -> Vec<(String, Vec<Component>)> {
    let mut out = Vec::new();
    let mut push = |item: &str, name: &str, value: Value| {
        if has(item) {
            out.push((item.to_owned(), vec![(name.to_owned(), value)]));
        }
    };
    for item in [
        "minecraft:potion",
        "minecraft:splash_potion",
        "minecraft:lingering_potion",
        "minecraft:tipped_arrow",
    ] {
        for potion in potions {
            push(
                item,
                "minecraft:potion_contents",
                json!({ "potion": potion }),
            );
        }
    }
    for (id, max) in enchantments {
        for level in 1..=(*max).max(1) {
            push(
                "minecraft:enchanted_book",
                "minecraft:stored_enchantments",
                json!({ id.clone(): level }),
            );
        }
    }
    for id in instruments {
        push("minecraft:goat_horn", "minecraft:instrument", json!(id));
    }
    for level in 0..5 {
        push(
            "minecraft:ominous_bottle",
            "minecraft:ominous_bottle_amplifier",
            json!(level),
        );
    }
    out
}

/// A brewing recipe's file (`data/<ns>/recipe/brewing/*.json`, type `minecraft:brewing`): the bottles it takes
/// (item ids, and the potions they must hold, if any), the reagent's items, and what it makes. Tag names
/// (`#…`) are left for the caller to expand.
#[derive(Clone, Debug, PartialEq)]
pub struct Brew {
    pub inputs: Vec<String>,
    pub input_potions: Vec<String>,
    pub reagents: Vec<String>,
    pub output: String,
    pub output_count: u32,
    pub output_components: Vec<Component>,
}

fn strings(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(a) => a
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

/// The brewing recipe a file describes; none for another type or one making nothing.
pub fn brew(json: &Value) -> Option<Brew> {
    if json.get("type").and_then(Value::as_str) != Some("minecraft:brewing") {
        return None;
    }
    let output = json.get("output")?;
    let id = output.get("id").and_then(Value::as_str)?.to_owned();
    let components = (output.get("components").and_then(Value::as_object))
        .map(|m| {
            let mut c: Vec<Component> = (m.iter())
                .filter(|(k, _)| subtype(k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            c.sort_by(|a, b| a.0.cmp(&b.0));
            c
        })
        .unwrap_or_default();
    let input = json.get("input").unwrap_or(&Value::Null);
    Some(Brew {
        inputs: strings(input.get("item").unwrap_or(&Value::Null)),
        input_potions: strings(
            (input.get("potion_contents"))
                .and_then(|p| p.get("potions"))
                .unwrap_or(&Value::Null),
        ),
        reagents: strings(
            (json.get("reagent").and_then(|r| r.get("item"))).unwrap_or(&Value::Null),
        ),
        output: id,
        output_count: output.get("count").and_then(Value::as_u64).unwrap_or(1) as u32,
        output_components: components,
    })
}

/// A potion's components, as a bottle that holds it carries them.
pub fn potion(potion: &str) -> Vec<Component> {
    vec![(
        "minecraft:potion_contents".to_owned(),
        json!({ "potion": potion }),
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_three_forms_of_a_potion() {
        let c = potion("minecraft:swiftness");
        assert_eq!(
            command(&c),
            r#"[minecraft:potion_contents={"potion":"minecraft:swiftness"}]"#
        );
        assert_eq!(
            json_form(&c),
            r#"{"minecraft:potion_contents":{"potion":"minecraft:swiftness"}}"#
        );
        assert_eq!(
            snbt("minecraft:potion", &c),
            r#"{id:"minecraft:potion",count:1,components:{"minecraft:potion_contents":{"potion":"minecraft:swiftness"}}}"#
        );
        assert_eq!(from_command(&command(&c)), c);
        // The runtime's subtype key keeps it.
        assert_eq!(crate::runtime::subtype_key(&command(&c)), command(&c));
    }

    #[test]
    fn the_games_snbt_spelling_is_the_same_variant() {
        let game =
            r#"[minecraft:damage=3,minecraft:potion_contents={potion:"minecraft:swiftness"}]"#;
        assert_eq!(canonical(game), command(&potion("minecraft:swiftness")));
        assert_eq!(
            canonical("[ominous_bottle_amplifier=2b]"),
            "[minecraft:ominous_bottle_amplifier=2]"
        );
        assert_eq!(
            canonical(r#"[minecraft:stored_enchantments={'minecraft:mending':1}]"#),
            r#"[minecraft:stored_enchantments={"minecraft:mending":1}]"#
        );
        assert!(from_command("[minecraft:damage=3,x=nope]").is_empty());
        assert!(from_command("").is_empty());
    }

    #[test]
    fn registries_make_variants_for_the_items_the_game_has() {
        let has = |id: &str| id != "minecraft:tipped_arrow";
        let v = registry_variants(
            &has,
            &["minecraft:water".into(), "minecraft:swiftness".into()],
            &[("minecraft:sharpness".into(), 5)],
            &["minecraft:ponder_goat_horn".into()],
        );
        // 3 bottle items x 2 potions, 5 books, 1 horn, 5 ominous bottles.
        assert_eq!(v.len(), 6 + 5 + 1 + 5);
        assert!(v.iter().all(|(item, _)| item != "minecraft:tipped_arrow"));
    }

    #[test]
    fn a_brewing_file_is_read() {
        let json: Value = mod_api::serde_json::from_str(
            r#"{"type":"minecraft:brewing",
                "input":{"item":["minecraft:potion","minecraft:splash_potion"],
                         "potion_contents":{"potions":"minecraft:awkward"}},
                "reagent":{"item":"minecraft:sugar"},
                "output":{"id":"minecraft:potion","components":
                          {"minecraft:potion_contents":{"potion":"minecraft:swiftness"}}}}"#,
        )
        .unwrap();
        let b = brew(&json).unwrap();
        assert_eq!(b.inputs, ["minecraft:potion", "minecraft:splash_potion"]);
        assert_eq!(b.input_potions, ["minecraft:awkward"]);
        assert_eq!(b.reagents, ["minecraft:sugar"]);
        assert_eq!(b.output_components, potion("minecraft:swiftness"));
        assert_eq!(brew(&json!({"type": "minecraft:smelting"})), None);
    }
}
