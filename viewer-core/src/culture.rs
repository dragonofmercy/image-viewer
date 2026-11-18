use std::collections::HashMap;

pub const AVAILABLE_LANGUAGES: [&str; 8] = ["en-US", "fr-FR", "zh-Hans", "de-DE", "es-ES", "it-IT", "pt-BR", "ru-RU"];

const DEFAULT_LANGUAGE: &str = "en-US";

const RESOURCES: [&str; 8] = [
    include_str!("../strings/en-US/Resources.resw"),
    include_str!("../strings/fr-FR/Resources.resw"),
    include_str!("../strings/zh-Hans/Resources.resw"),
    include_str!("../strings/de-DE/Resources.resw"),
    include_str!("../strings/es-ES/Resources.resw"),
    include_str!("../strings/it-IT/Resources.resw"),
    include_str!("../strings/pt-BR/Resources.resw"),
    include_str!("../strings/ru-RU/Resources.resw"),
];

const NATIVE_NAMES: [&str; 8] = [
    "English (United States)",
    "fran\u{e7}ais (France)",
    "\u{4e2d}\u{6587}(\u{7b80}\u{4f53})",
    "Deutsch (Deutschland)",
    "espa\u{f1}ol (Espa\u{f1}a)",
    "italiano (Italia)",
    "portugu\u{ea}s (Brasil)",
    "\u{440}\u{443}\u{441}\u{441}\u{43a}\u{438}\u{439} (\u{420}\u{43e}\u{441}\u{441}\u{438}\u{44f})",
];

fn index_of(tag: &str) -> Option<usize> {
    AVAILABLE_LANGUAGES.iter().position(|t| *t == tag)
}

/// Override wins if supported; otherwise the first system tag matching exactly or by primary subtag; otherwise en-US.
pub fn resolve_language(override_tag: &str, system_tags: &[String]) -> &'static str {
    if let Some(i) = index_of(override_tag) {
        return AVAILABLE_LANGUAGES[i];
    }
    for tag in system_tags {
        if let Some(found) = AVAILABLE_LANGUAGES.iter().find(|t| t.eq_ignore_ascii_case(tag)) {
            return found;
        }
        let primary = tag.split('-').next().unwrap_or("");
        if let Some(found) = AVAILABLE_LANGUAGES.iter().find(|t| t.split('-').next().unwrap_or("").eq_ignore_ascii_case(primary)) {
            return found;
        }
    }
    DEFAULT_LANGUAGE
}

/// Empty string for an unsupported tag.
pub fn native_name(tag: &str) -> &'static str {
    index_of(tag).map_or("", |i| NATIVE_NAMES[i])
}

fn parse(tag: &str) -> HashMap<String, String> {
    let Some(i) = index_of(tag) else { return HashMap::new() };
    let doc = roxmltree::Document::parse(RESOURCES[i]).expect("embedded .resw is valid XML");
    doc.root_element()
        .children()
        .filter(|n| n.has_tag_name("data"))
        .filter_map(|n| {
            let name = n.attribute("name")?;
            let value = n.children().find(|c| c.has_tag_name("value"))?.text().unwrap_or("");
            Some((name.to_string(), value.to_string()))
        })
        .collect()
}

pub fn keys_of(tag: &str) -> Vec<String> {
    parse(tag).into_keys().collect()
}

pub struct Strings {
    chosen: HashMap<String, String>,
    fallback: HashMap<String, String>,
}

impl Strings {
    pub fn load(tag: &str) -> Strings {
        Strings {
            chosen: parse(tag),
            fallback: parse(DEFAULT_LANGUAGE),
        }
    }

    pub fn get(&self, key: &str) -> String {
        self.chosen.get(key).or_else(|| self.fallback.get(key)).cloned().unwrap_or_else(|| format!("[{key}]"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_language_has_same_keys_as_english() {
        let mut en = keys_of("en-US");
        en.sort();
        for tag in AVAILABLE_LANGUAGES.iter().filter(|t| **t != "en-US") {
            let mut other = keys_of(tag);
            other.sort();
            assert_eq!(en, other, "{tag} key set differs from en-US");
        }
    }

    #[test]
    fn missing_key_returns_bracketed_key() {
        assert_eq!(Strings::load("fr-FR").get("NOPE"), "[NOPE]");
    }

    #[test]
    fn sample_key_matches_resw_content() {
        let expected = [
            ("en-US", "System language"),
            ("fr-FR", "Langue du système"),
            ("zh-Hans", "系统语言"),
            ("de-DE", "Systemsprache"),
            ("es-ES", "Idioma del sistema"),
            ("it-IT", "Lingua del sistema"),
            ("pt-BR", "Idioma do sistema"),
            ("ru-RU", "Язык системы"),
        ];
        for (tag, value) in expected {
            assert_eq!(Strings::load(tag).get("DEFAULT_SYSTEM_LANGUAGE"), value, "{tag}");
        }
    }

    #[test]
    fn resolve_language_cases() {
        let sys = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(resolve_language("fr-FR", &[]), "fr-FR");
        assert_eq!(resolve_language("", &sys(&["fr-CA"])), "fr-FR");
        assert_eq!(resolve_language("", &sys(&["zh-CN"])), "zh-Hans");
        assert_eq!(resolve_language("", &sys(&["ja-JP"])), "en-US");
        assert_eq!(resolve_language("xx", &sys(&["de-DE"])), "de-DE");
        assert_eq!(resolve_language("", &sys(&["PT-pt"])), "pt-BR");
        assert_eq!(resolve_language("", &sys(&["ja-JP", "de-AT"])), "de-DE");
        assert_eq!(resolve_language("", &sys(&["DE-de"])), "de-DE");
    }

    #[test]
    fn native_names() {
        assert_eq!(native_name("fr-FR"), "français (France)");
        assert_eq!(native_name("zh-Hans"), "中文(简体)");
        assert_eq!(native_name("zz"), "");
    }
}
