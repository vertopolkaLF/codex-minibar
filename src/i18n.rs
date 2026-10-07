//! Embedded Fluent catalogs. English is the source and per-message fallback.
//!
//! Static messages are resolved once for each supported language; returning a
//! stable reference keeps the existing UI builders and borrowed labels simple.
//! Parameterized messages are resolved on the calling thread, never translated
//! by matching rendered text (account names, paths and API data stay untouched).
use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::HashMap,
    sync::{
        LazyLock,
        atomic::{AtomicU8, Ordering},
    },
};

const EN: &str = include_str!("../locales/en/app.ftl");
const RU: &str = include_str!("../locales/ru/app.ftl");

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    Auto,
    English,
    Russian,
}

static CURRENT: AtomicU8 = AtomicU8::new(1);

impl Language {
    pub fn apply(self) {
        let russian = match self {
            Self::Russian => true,
            Self::English => false,
            Self::Auto => system_language_is_russian(),
        };
        CURRENT.store(if russian { 2 } else { 1 }, Ordering::Release);
    }
    pub fn index(self) -> usize {
        match self {
            Self::Auto => 0,
            Self::English => 1,
            Self::Russian => 2,
        }
    }
    pub fn from_index(index: usize) -> Self {
        match index {
            1 => Self::English,
            2 => Self::Russian,
            _ => Self::Auto,
        }
    }
}

pub fn is_russian() -> bool {
    CURRENT.load(Ordering::Acquire) == 2
}

fn system_language_is_russian() -> bool {
    #[cfg(windows)]
    {
        // UI language, not the regional date/number format or keyboard layout.
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetUserDefaultUILanguage() -> u16;
        }
        unsafe { GetUserDefaultUILanguage() & 0x03ff == 0x19 }
    }
    #[cfg(not(windows))]
    {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|name| std::env::var(name).ok().filter(|s| !s.is_empty()))
            .is_some_and(|s| s.to_ascii_lowercase().starts_with("ru"))
    }
}

fn bundle(language: &str, source: &str) -> FluentBundle<FluentResource> {
    let resource =
        FluentResource::try_new(source.to_owned()).expect("valid embedded Fluent catalog");
    let mut bundle = FluentBundle::new(vec![language.parse().expect("supported locale")]);
    // Desktop labels do not need bidi isolation around numeric substitutions.
    bundle.set_use_isolating(false);
    bundle
        .add_resource(resource)
        .expect("unique Fluent message IDs");
    bundle
}

struct Catalogs {
    en: FluentBundle<FluentResource>,
    ru: FluentBundle<FluentResource>,
}
impl Catalogs {
    fn new() -> Self {
        Self {
            en: bundle("en", EN),
            ru: bundle("ru", RU),
        }
    }
    fn resolve(&self, russian: bool, key: &str, args: Option<&FluentArgs>) -> Option<String> {
        let resolve = |bundle: &FluentBundle<FluentResource>| {
            let message = bundle.get_message(key)?;
            let pattern = message.value()?;
            let mut errors = Vec::new();
            let value = bundle
                .format_pattern(pattern, args, &mut errors)
                .into_owned();
            (errors.is_empty() && !value.trim().is_empty()).then_some(value)
        };
        if russian {
            resolve(&self.ru).or_else(|| resolve(&self.en))
        } else {
            resolve(&self.en)
        }
    }
}

thread_local! { static CATALOGS: RefCell<Catalogs> = RefCell::new(Catalogs::new()); }

static LABELS: LazyLock<HashMap<String, (&'static str, &'static str)>> = LazyLock::new(|| {
    let catalogs = Catalogs::new();
    let mut labels = HashMap::new();
    for line in EN
        .lines()
        .filter(|line| !line.starts_with([' ', '#']) && line.contains(" = "))
    {
        let key = line.split(" = ").next().unwrap();
        if let Some(en) = catalogs.resolve(false, key, None) {
            let ru = catalogs
                .resolve(true, key, None)
                .unwrap_or_else(|| en.clone());
            labels.insert(
                key.to_owned(),
                (
                    Box::leak(en.into_boxed_str()) as &'static str,
                    Box::leak(ru.into_boxed_str()) as &'static str,
                ),
            );
        }
    }
    labels
});

/// Resolve a static label with English fallback. Keys are checked in tests.
pub fn tr(key: &'static str) -> &'static str {
    LABELS
        .get(key)
        .map(|(en, ru)| if is_russian() { *ru } else { *en })
        .unwrap_or(key)
}

/// Keep stored errors in the source language so login detection and future
/// language switches are independent of the language at the time of a poll.
pub fn english(key: &'static str) -> &'static str {
    LABELS.get(key).map(|(en, _)| *en).unwrap_or(key)
}

/// Resolve an explicit language without changing the process language. Also
/// used by a second process to find windows opened in either language.
pub fn tr_in(language: Language, key: &'static str) -> &'static str {
    let russian = match language {
        Language::Russian => true,
        Language::English => false,
        Language::Auto => system_language_is_russian(),
    };
    LABELS
        .get(key)
        .map(|(en, ru)| if russian { *ru } else { *en })
        .unwrap_or(key)
}

/// This boundary is only for application error summaries, never user labels.
pub fn localize_error(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            LABELS
                .values()
                .find(|(en, _)| *en == line)
                .map(|(en, ru)| if is_russian() { *ru } else { *en })
                .unwrap_or(line)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Locale-aware textual dates; numeric clock/date preferences stay independent.
pub fn month_day(date: impl chrono::Datelike) -> String {
    let month = tr([
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ][(date.month() - 1) as usize]);
    if is_russian() {
        format!("{} {month}", date.day())
    } else {
        format!("{month} {}", date.day())
    }
}

pub fn weekday(day: chrono::Weekday) -> &'static str {
    tr(["mon", "tue", "wed", "thu", "fri", "sat", "sun"][day.num_days_from_monday() as usize])
}

pub fn date_with_year(date: impl chrono::Datelike + Copy) -> String {
    format!("{}, {}", month_day(date), date.year())
}

/// Translate a full message; parameters are data, never lookup keys.
pub fn format(key: &str, values: &[(&str, String)]) -> String {
    let mut args = FluentArgs::new();
    for (name, value) in values {
        let plural_parameter = matches!(
            (key, *name),
            (
                "requests" | "requests-priced" | "sessions" | "sessions-0e5e29" | "api-key-count",
                "v0"
            ) | ("count-banked-resets", "count")
                | ("name-login-expires-in-days-left", "days_left")
        );
        if plural_parameter && let Ok(number) = value.parse::<u64>() {
            args.set(*name, number);
        } else {
            args.set(*name, value.as_str());
        }
    }
    CATALOGS
        .with(|catalogs| catalogs.borrow().resolve(is_russian(), key, Some(&args)))
        .unwrap_or_else(|| key.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};
    use std::collections::{BTreeMap, BTreeSet};

    fn inline_variables(expression: &InlineExpression<&str>, result: &mut BTreeSet<String>) {
        match expression {
            InlineExpression::VariableReference { id } => {
                result.insert(id.name.into());
            }
            InlineExpression::Placeable { expression } => expression_variables(expression, result),
            _ => {}
        }
    }
    fn expression_variables(expression: &Expression<&str>, result: &mut BTreeSet<String>) {
        match expression {
            Expression::Inline(inline) => inline_variables(inline, result),
            Expression::Select { selector, variants } => {
                inline_variables(selector, result);
                for variant in variants {
                    pattern_variables(&variant.value, result);
                }
            }
        }
    }
    fn pattern_variables(pattern: &Pattern<&str>, result: &mut BTreeSet<String>) {
        for element in &pattern.elements {
            if let PatternElement::Placeable { expression } = element {
                expression_variables(expression, result);
            }
        }
    }
    fn catalog_parameters(source: &str) -> BTreeMap<String, BTreeSet<String>> {
        let resource = fluent_syntax::parser::parse(source).expect("valid Fluent syntax");
        let mut messages = BTreeMap::new();
        for entry in resource.body {
            if let Entry::Message(message) = entry {
                let mut variables = BTreeSet::new();
                pattern_variables(
                    message.value.as_ref().expect("message value"),
                    &mut variables,
                );
                assert!(
                    messages.insert(message.id.name.into(), variables).is_none(),
                    "duplicate message ID"
                );
            }
        }
        messages
    }

    #[test]
    fn catalogs_cover_the_same_messages_and_parameters_and_resolve_without_errors() {
        let en = catalog_parameters(EN);
        let ru = catalog_parameters(RU);
        assert_eq!(
            en, ru,
            "Russian must cover every English key and preserve its parameters"
        );
        let template = catalog_parameters(include_str!("../locales/template/app.ftl"));
        assert_eq!(
            en.keys().collect::<Vec<_>>(),
            template.keys().collect::<Vec<_>>()
        );
        let catalogs = Catalogs::new();
        for (key, parameters) in &en {
            let mut args = FluentArgs::new();
            for name in parameters {
                args.set(name.as_str(), 2_u64);
            }
            assert!(
                catalogs.resolve(false, key, Some(&args)).is_some(),
                "English: {key}"
            );
            // Check Russian directly, so fallback cannot hide a broken translation.
            let message = catalogs.ru.get_message(key).unwrap();
            let mut errors = Vec::new();
            let value =
                catalogs
                    .ru
                    .format_pattern(message.value().unwrap(), Some(&args), &mut errors);
            assert!(
                errors.is_empty() && !value.trim().is_empty(),
                "Russian: {key}: {errors:?}"
            );
        }
    }

    #[test]
    fn every_literal_translation_call_has_an_english_message() {
        fn walk(path: &std::path::Path, keys: &BTreeMap<String, BTreeSet<String>>) {
            for entry in std::fs::read_dir(path).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    walk(&path, keys);
                    continue;
                }
                if path.extension().is_none_or(|ext| ext != "rs")
                    || path.file_name().unwrap() == "i18n.rs"
                {
                    continue;
                }
                let source = std::fs::read_to_string(&path).unwrap();
                for call in source.split("crate::i18n::").skip(1) {
                    let Some((function, argument)) = call.split_once('(') else {
                        continue;
                    };
                    if !matches!(function, "tr" | "format" | "english") {
                        continue;
                    }
                    if let Some(literal) = argument.trim_start().strip_prefix('"') {
                        let key = literal.split('"').next().unwrap();
                        assert!(keys.contains_key(key), "{}: {key}", path.display());
                    }
                }
            }
        }
        walk(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
            &catalog_parameters(EN),
        );
    }

    #[test]
    fn russian_uses_plural_rules_for_one_few_many_and_twenty_one() {
        let catalogs = Catalogs::new();
        for (number, expected) in [
            (1, "1 API-ключ"),
            (2, "2 API-ключа"),
            (5, "5 API-ключей"),
            (21, "21 API-ключ"),
        ] {
            let mut args = FluentArgs::new();
            args.set("v0", number);
            assert_eq!(
                catalogs
                    .resolve(true, "api-key-count", Some(&args))
                    .as_deref(),
                Some(expected)
            );
        }
    }

    #[test]
    fn parameters_are_never_translated_or_treated_as_message_keys() {
        let catalogs = Catalogs::new();
        let mut args = FluentArgs::new();
        args.set("name", "Settings");
        assert_eq!(
            catalogs
                .resolve(true, "name-deleted", Some(&args))
                .as_deref(),
            Some("Settings удалён.")
        );
    }

    #[test]
    fn old_settings_default_to_auto_and_explicit_language_round_trips() {
        let settings: crate::settings::Settings = toml::from_str("version = 1").unwrap();
        assert_eq!(settings.language, Language::Auto);
        for language in [Language::Auto, Language::English, Language::Russian] {
            let settings = crate::settings::Settings {
                language,
                ..Default::default()
            };
            let encoded = toml::to_string(&settings).unwrap();
            let decoded: crate::settings::Settings = toml::from_str(&encoded).unwrap();
            assert_eq!(decoded.language, language);
        }
    }
    #[test]
    fn metric_labels_keep_standalone_and_sentence_forms_separate() {
        assert_eq!(tr_in(Language::Russian, "cost"), "Расход");
        assert_eq!(tr_in(Language::Russian, "tokens"), "Токены");
        assert_eq!(tr_in(Language::Russian, "cost-885dc4"), "расхода");
        assert_eq!(tr_in(Language::Russian, "tokens-339143"), "токенов");
    }

    #[test]
    fn missing_empty_or_invalid_russian_message_falls_back_to_english() {
        let catalogs = Catalogs {
            en: bundle(
                "en",
                "missing = English\nempty = English\nbad = Hello { $name }\n",
            ),
            ru: bundle("ru", "empty = { \"\" }\nbad = Привет { $unknown }\n"),
        };
        let mut args = FluentArgs::new();
        args.set("name", "Ada");
        assert_eq!(
            catalogs.resolve(true, "missing", None).as_deref(),
            Some("English")
        );
        assert_eq!(
            catalogs.resolve(true, "empty", None).as_deref(),
            Some("English")
        );
        assert_eq!(
            catalogs.resolve(true, "bad", Some(&args)).as_deref(),
            Some("Hello Ada")
        );
    }
}
