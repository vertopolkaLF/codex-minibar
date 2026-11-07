//! Embedded Fluent catalogs. English is the source and per-message fallback.
//!
//! Static messages are resolved once for each supported language, so lookups
//! return `&'static str` and labels can be borrowed freely.
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

use crate::choice::Choice;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    #[default]
    Auto,
    English,
    Russian,
    BrazilianPortuguese,
    Spanish,
    SimplifiedChinese,
    Japanese,
    German,
    French,
    TraditionalChinese,
    Korean,
    Turkish,
}

/// One supported display language and its embedded catalog.
struct Locale {
    language: Language,
    /// Shown in every language selector.
    native_name: &'static str,
    /// The catalog's BCP 47 tag.
    tag: &'static str,
    /// Primary subtag of a system locale tag that selects this language.
    primary_subtag: &'static str,
    /// Windows primary language id that selects this language.
    primary_langid: u16,
    catalog: &'static str,
}

/// Native names in alphabetical (Unicode) order, shared by every selector.
/// Simplified Chinese precedes Traditional, so a bare `zh` selects it.
const LOCALES: [Locale; 11] = [
    Locale {
        language: Language::German,
        native_name: "Deutsch",
        tag: "de",
        primary_subtag: "de",
        primary_langid: 0x07,
        catalog: include_str!("../locales/de/app.ftl"),
    },
    Locale {
        language: Language::English,
        native_name: "English",
        tag: "en",
        primary_subtag: "en",
        primary_langid: 0x09,
        catalog: include_str!("../locales/en/app.ftl"),
    },
    Locale {
        language: Language::Spanish,
        native_name: "Español",
        tag: "es",
        primary_subtag: "es",
        primary_langid: 0x0a,
        catalog: include_str!("../locales/es/app.ftl"),
    },
    Locale {
        language: Language::French,
        native_name: "Français",
        tag: "fr",
        primary_subtag: "fr",
        primary_langid: 0x0c,
        catalog: include_str!("../locales/fr/app.ftl"),
    },
    Locale {
        language: Language::BrazilianPortuguese,
        native_name: "Português (Brasil)",
        tag: "pt-BR",
        primary_subtag: "pt",
        primary_langid: 0x16,
        catalog: include_str!("../locales/pt-BR/app.ftl"),
    },
    Locale {
        language: Language::Turkish,
        native_name: "Türkçe",
        tag: "tr",
        primary_subtag: "tr",
        primary_langid: 0x1f,
        catalog: include_str!("../locales/tr/app.ftl"),
    },
    Locale {
        language: Language::Russian,
        native_name: "Русский",
        tag: "ru",
        primary_subtag: "ru",
        primary_langid: 0x19,
        catalog: include_str!("../locales/ru/app.ftl"),
    },
    Locale {
        language: Language::Japanese,
        native_name: "日本語",
        tag: "ja",
        primary_subtag: "ja",
        primary_langid: 0x11,
        catalog: include_str!("../locales/ja/app.ftl"),
    },
    Locale {
        language: Language::SimplifiedChinese,
        native_name: "简体中文",
        tag: "zh-CN",
        primary_subtag: "zh",
        primary_langid: 0x04,
        catalog: include_str!("../locales/zh-CN/app.ftl"),
    },
    Locale {
        language: Language::TraditionalChinese,
        native_name: "繁體中文",
        tag: "zh-TW",
        primary_subtag: "zh",
        primary_langid: 0x04,
        catalog: include_str!("../locales/zh-TW/app.ftl"),
    },
    Locale {
        language: Language::Korean,
        native_name: "한국어",
        tag: "ko-KR",
        primary_subtag: "ko",
        primary_langid: 0x12,
        catalog: include_str!("../locales/ko-KR/app.ftl"),
    },
];

/// Position of `language` in [`LOCALES`]; Auto and unknown values map to English.
const fn locale_index(language: Language) -> usize {
    let mut index = 0;
    while index < LOCALES.len() {
        if LOCALES[index].language as u8 == language as u8 {
            return index;
        }
        index += 1;
    }
    ENGLISH
}

const ENGLISH: usize = 1;
const _: () = assert!(LOCALES[ENGLISH].language as u8 == Language::English as u8);
const EN: &str = LOCALES[ENGLISH].catalog;

/// Auto first, then [`LOCALES`] in order.
const SELECTABLE: [Language; LOCALES.len() + 1] = {
    let mut languages = [Language::Auto; LOCALES.len() + 1];
    let mut index = 0;
    while index < LOCALES.len() {
        languages[index + 1] = LOCALES[index].language;
        index += 1;
    }
    languages
};

/// Index into [`LOCALES`] of the resolved process language.
static CURRENT: AtomicU8 = AtomicU8::new(ENGLISH as u8);

impl Choice for Language {
    const ALL: &'static [Self] = &SELECTABLE;
}

impl Language {
    pub fn supported() -> impl Iterator<Item = Self> {
        LOCALES.iter().map(|locale| locale.language)
    }

    pub fn apply(self) {
        CURRENT.store(locale_index(self.resolved()) as u8, Ordering::Release);
    }

    pub fn resolved(self) -> Self {
        if self == Self::Auto {
            system_language()
        } else {
            self
        }
    }

    /// Selector labels in [`Choice::ALL`] order.
    pub fn labels() -> Vec<&'static str> {
        std::iter::once(tr("auto-windows"))
            .chain(LOCALES.iter().map(|locale| locale.native_name))
            .collect()
    }
}

pub fn current_language() -> Language {
    LOCALES[usize::from(CURRENT.load(Ordering::Acquire))].language
}

fn language_from_tag(tag: &str) -> Language {
    let normalized = tag.to_ascii_lowercase().replace('_', "-");
    let mut subtags = normalized.split(['-', '.', '@']);
    let primary = subtags.next().unwrap_or("");
    if primary == "zh" {
        let subtags = subtags.collect::<Vec<_>>();
        // An explicit script takes precedence over a region (e.g. zh-Hans-HK).
        if subtags.contains(&"hant") {
            return Language::TraditionalChinese;
        }
        if subtags.contains(&"hans") {
            return Language::SimplifiedChinese;
        }
        if subtags
            .iter()
            .any(|part| matches!(*part, "tw" | "hk" | "mo"))
        {
            return Language::TraditionalChinese;
        }
    }
    LOCALES
        .iter()
        .find(|locale| locale.primary_subtag == primary)
        .map_or(Language::English, |locale| locale.language)
}

#[cfg(any(windows, test))]
fn language_from_windows_langid(langid: u16) -> Language {
    if matches!(langid, 0x0404 | 0x0c04 | 0x1404 | 0x7c04) {
        return Language::TraditionalChinese;
    }
    language_from_tag(
        LOCALES
            .iter()
            .find(|locale| locale.primary_langid == langid & 0x03ff)
            .map_or("en", |locale| locale.primary_subtag),
    )
}

fn system_language() -> Language {
    #[cfg(windows)]
    {
        // UI language, not the regional date/number format or keyboard layout.
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetUserDefaultUILanguage() -> u16;
        }
        language_from_windows_langid(unsafe { GetUserDefaultUILanguage() })
    }
    #[cfg(not(windows))]
    {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|name| std::env::var(name).ok().filter(|s| !s.is_empty()))
            .map(|tag| language_from_tag(&tag))
            .unwrap_or(Language::English)
    }
}

/// Layout QA only: `CODEX_MINIBAR_PSEUDO_LOCALE=1` accents English copy and
/// makes it ~40% longer, like long translations such as Portuguese or German.
const PSEUDO_LOCALE_ENV: &str = "CODEX_MINIBAR_PSEUDO_LOCALE";

/// Transforms literal Fluent text only; placeables such as numbers stay intact.
fn pseudo_localize(text: &str) -> std::borrow::Cow<'_, str> {
    let letters = text.chars().filter(|c| c.is_alphabetic()).count();
    if letters == 0 {
        return text.into();
    }
    let mut result: String = text
        .chars()
        .map(|c| match c {
            'a' => 'á',
            'e' => 'é',
            'i' => 'í',
            'o' => 'ó',
            'u' => 'ú',
            'c' => 'ç',
            'n' => 'ñ',
            'A' => 'Á',
            'E' => 'É',
            'O' => 'Ó',
            'U' => 'Ú',
            other => other,
        })
        .collect();
    // Pad inside the trailing whitespace so spacing around placeables stays.
    let trimmed = result.trim_end().len();
    let padding = "ẋ".repeat(letters.div_ceil(5) * 2);
    result.insert_str(trimmed, &padding);
    result.into()
}

fn bundle(language: &str, source: &str) -> FluentBundle<FluentResource> {
    let resource =
        FluentResource::try_new(source.to_owned()).expect("valid embedded Fluent catalog");
    let mut bundle = FluentBundle::new(vec![language.parse().expect("supported locale")]);
    // Desktop labels do not need bidi isolation around numeric substitutions.
    bundle.set_use_isolating(false);
    if language == "en" && std::env::var_os(PSEUDO_LOCALE_ENV).is_some_and(|value| value == "1") {
        bundle.set_transform(Some(pseudo_localize));
    }
    bundle
        .add_resource(resource)
        .expect("unique Fluent message IDs");
    bundle
}

/// One bundle per [`LOCALES`] entry, in the same order.
struct Catalogs {
    bundles: Vec<FluentBundle<FluentResource>>,
}
impl Catalogs {
    fn new() -> Self {
        Self {
            bundles: LOCALES
                .iter()
                .map(|locale| bundle(locale.tag, locale.catalog))
                .collect(),
        }
    }
    fn for_language(&self, language: Language) -> &FluentBundle<FluentResource> {
        &self.bundles[locale_index(language.resolved())]
    }
    fn resolve(&self, language: Language, key: &str, args: Option<&FluentArgs>) -> Option<String> {
        let resolve = |bundle: &FluentBundle<FluentResource>| {
            let message = bundle.get_message(key)?;
            let pattern = message.value()?;
            let mut errors = Vec::new();
            let value = bundle
                .format_pattern(pattern, args, &mut errors)
                .into_owned();
            (errors.is_empty() && !value.trim().is_empty()).then_some(value)
        };
        resolve(self.for_language(language)).or_else(|| resolve(&self.bundles[ENGLISH]))
    }
}

thread_local! { static CATALOGS: RefCell<Catalogs> = RefCell::new(Catalogs::new()); }

/// Every static message, resolved for each [`LOCALES`] entry in order.
static LABELS: LazyLock<HashMap<String, [&'static str; LOCALES.len()]>> = LazyLock::new(|| {
    let catalogs = Catalogs::new();
    let mut labels = HashMap::new();
    for line in EN
        .lines()
        .filter(|line| !line.starts_with([' ', '#']) && line.contains(" = "))
    {
        let key = line.split(" = ").next().unwrap();
        if catalogs.resolve(Language::English, key, None).is_some() {
            let values = LOCALES.each_ref().map(|locale| {
                Box::leak(
                    catalogs
                        .resolve(locale.language, key, None)
                        .expect("English fallback")
                        .into_boxed_str(),
                ) as &'static str
            });
            labels.insert(key.to_owned(), values);
        }
    }
    labels
});

/// Resolve a static label with English fallback. Keys are checked in tests.
pub fn tr(key: &'static str) -> &'static str {
    tr_in(current_language(), key)
}

/// Keep stored errors in the source language so login detection and future
/// language switches are independent of the language at the time of a poll.
pub fn english(key: &'static str) -> &'static str {
    LABELS.get(key).map(|labels| labels[ENGLISH]).unwrap_or(key)
}

/// Resolve an explicit language without changing the process language. Also
/// used by a second process to find windows opened in any supported language.
pub fn tr_in(language: Language, key: &'static str) -> &'static str {
    LABELS
        .get(key)
        .map(|labels| labels[locale_index(language.resolved())])
        .unwrap_or(key)
}

/// This boundary is only for application error summaries, never user labels.
pub fn localize_error(source: &str) -> String {
    let index = usize::from(CURRENT.load(Ordering::Acquire));
    source
        .lines()
        .map(|line| {
            LABELS
                .values()
                .find(|labels| labels[ENGLISH] == line)
                .map(|labels| labels[index])
                .unwrap_or(line)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Locale-aware textual dates; numeric clock/date preferences stay independent.
pub fn month_day(date: impl chrono::Datelike) -> String {
    month_day_in(current_language(), date)
}

fn month_day_in(language: Language, date: impl chrono::Datelike) -> String {
    let month = tr_in(
        language,
        [
            "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
        ][(date.month() - 1) as usize],
    );
    match language {
        Language::Russian | Language::French | Language::Turkish => {
            format!("{} {month}", date.day())
        }
        Language::German => format!("{}. {month}", date.day()),
        Language::BrazilianPortuguese | Language::Spanish => format!("{} de {month}", date.day()),
        Language::Korean => format!("{month} {}일", date.day()),
        Language::SimplifiedChinese | Language::TraditionalChinese | Language::Japanese => {
            format!("{month}{}日", date.day())
        }
        _ => format!("{month} {}", date.day()),
    }
}

pub fn weekday(day: chrono::Weekday) -> &'static str {
    tr(["mon", "tue", "wed", "thu", "fri", "sat", "sun"][day.num_days_from_monday() as usize])
}

pub fn date_with_year(date: impl chrono::Datelike + Copy) -> String {
    date_with_year_in(current_language(), date)
}

fn date_with_year_in(language: Language, date: impl chrono::Datelike + Copy) -> String {
    match language {
        Language::Korean => format!("{}년 {}", date.year(), month_day_in(language, date)),
        Language::SimplifiedChinese | Language::TraditionalChinese | Language::Japanese => {
            format!("{}年{}", date.year(), month_day_in(language, date))
        }
        Language::German | Language::French | Language::Turkish => {
            format!("{} {}", month_day_in(language, date), date.year())
        }
        Language::BrazilianPortuguese | Language::Spanish => {
            format!("{} de {}", month_day_in(language, date), date.year())
        }
        _ => format!("{}, {}", month_day_in(language, date), date.year()),
    }
}

/// Translate a full message; parameters are data, never lookup keys.
pub fn format(key: &str, values: &[(&str, String)]) -> String {
    format_in(current_language(), key, values)
}

fn format_in(language: Language, key: &str, values: &[(&str, String)]) -> String {
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
        .with(|catalogs| catalogs.borrow().resolve(language, key, Some(&args)))
        .unwrap_or_else(|| key.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fluent_syntax::ast::{Entry, Expression, InlineExpression, Pattern, PatternElement};
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn native_language_names_are_sorted_and_match_their_selection_indices() {
        let labels = Language::labels();
        assert!(labels[1..].windows(2).all(|pair| pair[0] < pair[1]));
        for (offset, language) in Language::supported().enumerate() {
            assert_eq!(language.index(), offset + 1);
            assert_eq!(Language::from_index(offset + 1), language);
        }
        assert_eq!(labels[Language::English.index()], "English");
        assert_eq!(labels[Language::Turkish.index()], "Türkçe");
        assert_eq!(labels[Language::Russian.index()], "Русский");
        // Reordering the selector must not change source-language diagnostics.
        assert_eq!(english("settings"), "Settings");
        assert_eq!(tr_in(Language::English, "settings"), "Settings");
        assert_eq!(tr_in(Language::German, "settings"), "Einstellungen");
    }

    #[test]
    fn turkish_detection_dates_counts_and_settings() {
        for tag in ["tr", "tr-TR", "TR_tr.UTF-8", "tr-CY"] {
            assert_eq!(language_from_tag(tag), Language::Turkish);
        }
        for langid in [0x041f, 0x001f] {
            assert_eq!(language_from_windows_langid(langid), Language::Turkish);
        }
        let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 8).unwrap();
        assert_eq!(month_day_in(Language::Turkish, date), "8 Eki");
        assert_eq!(date_with_year_in(Language::Turkish, date), "8 Eki 2026");
        assert_eq!(tr_in(Language::Turkish, "settings"), "Ayarlar");
        assert_eq!(
            tr_in(Language::Turkish, "codex-minibar-settings"),
            "Codex Minibar Ayarları"
        );
        for number in [0, 1, 2, 5, 21, 1_000_000] {
            assert_eq!(
                format_in(Language::Turkish, "requests", &[("v0", number.to_string())]),
                format!("{number} istek")
            );
        }
        assert_eq!(
            format_in(
                Language::Turkish,
                "name-deleted",
                &[("name", "İş hesabı 🦀".into())]
            ),
            "İş hesabı 🦀 silindi."
        );
        let settings: crate::settings::Settings = toml::from_str("language = 'turkish'").unwrap();
        assert_eq!(settings.language, Language::Turkish);
        assert!(
            toml::to_string(&settings)
                .unwrap()
                .contains("language = \"turkish\"")
        );
    }

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
    fn pseudo_locale_lengthens_text_and_keeps_placeable_spacing() {
        assert_eq!(pseudo_localize("Resets in "), "Réséts íñẋẋẋẋ ");
        assert_eq!(pseudo_localize(" / "), " / ");
        let mut bundle = bundle("en", "msg = Expires in { $time }");
        bundle.set_transform(Some(pseudo_localize));
        let message = bundle.get_message("msg").unwrap();
        let mut args = FluentArgs::new();
        args.set("time", "6 d 23 h");
        let mut errors = Vec::new();
        let value = bundle.format_pattern(message.value().unwrap(), Some(&args), &mut errors);
        assert!(errors.is_empty());
        assert_eq!(value, "Éxpírés íñẋẋẋẋ 6 d 23 h");
    }

    #[test]
    fn catalogs_cover_the_same_messages_and_parameters_and_resolve_without_errors() {
        let en = catalog_parameters(EN);
        for locale in LOCALES
            .iter()
            .filter(|locale| locale.language != Language::English)
        {
            let (language, source) = (locale.language, locale.catalog);
            assert_eq!(
                en,
                catalog_parameters(source),
                "{language:?} must cover every English key and preserve its parameters"
            );
        }
        let template = catalog_parameters(include_str!("../locales/template/app.ftl"));
        assert_eq!(
            en.keys().collect::<Vec<_>>(),
            template.keys().collect::<Vec<_>>()
        );
        let catalogs = Catalogs::new();
        for language in Language::supported() {
            let catalog = catalogs.for_language(language);
            for (key, parameters) in &en {
                // Exercise every plural branch, including Portuguese zero and
                // large-number categories. Read directly: no English fallback.
                for number in [0_u64, 1, 2, 5, 21, 1_000_000] {
                    let mut args = FluentArgs::new();
                    for name in parameters {
                        args.set(name.as_str(), number);
                    }
                    let message = catalog.get_message(key).unwrap();
                    let mut errors = Vec::new();
                    let value =
                        catalog.format_pattern(message.value().unwrap(), Some(&args), &mut errors);
                    assert!(
                        errors.is_empty() && !value.trim().is_empty(),
                        "{language:?}: {key}: {number}: {errors:?}"
                    );
                }
            }
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
                    .resolve(Language::Russian, "api-key-count", Some(&args))
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
                .resolve(Language::Russian, "name-deleted", Some(&args))
                .as_deref(),
            Some("Settings удалён.")
        );
    }

    #[test]
    fn old_settings_default_to_auto_and_explicit_language_round_trips() {
        let settings: crate::settings::Settings = toml::from_str("version = 1").unwrap();
        assert_eq!(settings.language, Language::Auto);
        for language in Language::ALL.iter().copied() {
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
    fn missing_empty_or_invalid_translation_falls_back_to_english() {
        let catalogs = Catalogs {
            bundles: LOCALES
                .iter()
                .map(|locale| {
                    let source = match locale.language {
                        Language::English => {
                            "missing = English\nempty = English\nbad = Hello { $name }\n"
                        }
                        Language::Russian => "empty = { \"\" }\nbad = Привет { $unknown }\n",
                        _ => "empty = { \"\" }\nbad = Hello { $unknown }\n",
                    };
                    bundle(locale.tag, source)
                })
                .collect(),
        };
        let mut args = FluentArgs::new();
        args.set("name", "Ada");
        for language in Language::supported().filter(|language| *language != Language::English) {
            assert_eq!(
                catalogs.resolve(language, "missing", None).as_deref(),
                Some("English")
            );
            assert_eq!(
                catalogs.resolve(language, "empty", None).as_deref(),
                Some("English")
            );
            assert_eq!(
                catalogs.resolve(language, "bad", Some(&args)).as_deref(),
                Some("Hello Ada")
            );
        }
    }

    #[test]
    fn traditional_chinese_display_languages_do_not_use_simplified_chinese() {
        for tag in ["zh-TW", "zh_HK.UTF-8", "zh-MO", "zh-Hant", "ZH-hant-TW"] {
            assert_eq!(
                language_from_tag(tag),
                Language::TraditionalChinese,
                "{tag}"
            );
        }
    }

    #[test]
    fn windows_display_language_preserves_chinese_script() {
        for langid in [0x0404, 0x0c04, 0x1404, 0x7c04] {
            assert_eq!(
                language_from_windows_langid(langid),
                Language::TraditionalChinese
            );
        }
        for langid in [0x0004, 0x0804, 0x1004, 0x7804] {
            assert_eq!(
                language_from_windows_langid(langid),
                Language::SimplifiedChinese
            );
        }
        assert_eq!(language_from_windows_langid(0x0409), Language::English);
        assert_eq!(language_from_windows_langid(0x0411), Language::Japanese);
        assert_eq!(language_from_windows_langid(0x0412), Language::Korean);
        assert_eq!(language_from_windows_langid(0x0012), Language::Korean);
    }

    #[test]
    fn language_selection_indices_and_display_language_tags() {
        for language in Language::ALL.iter().copied() {
            assert_eq!(Language::from_index(language.index()), language);
        }
        assert_eq!(Language::from_index(usize::MAX), Language::Auto);
        assert_eq!(Language::labels().len(), Language::ALL.len());
        for (tag, expected) in [
            ("pt-BR", Language::BrazilianPortuguese),
            ("pt_PT.UTF-8", Language::BrazilianPortuguese),
            ("ES-mx", Language::Spanish),
            ("zh_CN.UTF-8", Language::SimplifiedChinese),
            ("zh-Hans", Language::SimplifiedChinese),
            ("zh-Hans-HK", Language::SimplifiedChinese),
            ("zh-Hant-CN", Language::TraditionalChinese),
            ("ru_RU", Language::Russian),
            ("ja-JP", Language::Japanese),
            ("ja_JP.UTF-8", Language::Japanese),
            ("de-DE", Language::German),
            ("de_AT.UTF-8", Language::German),
            ("fr-FR", Language::French),
            ("fr_CA.UTF-8", Language::French),
            ("ko-KR", Language::Korean),
            ("ko_KR.UTF-8", Language::Korean),
            ("KO", Language::Korean),
            ("ko-KP", Language::Korean),
            ("it-IT", Language::English),
            ("C", Language::English),
            ("", Language::English),
        ] {
            assert_eq!(language_from_tag(tag), expected, "{tag}");
        }
    }

    #[test]
    fn new_locales_resolve_static_labels_and_window_titles() {
        for (language, label, title) in [
            (
                Language::BrazilianPortuguese,
                "Configurações",
                "Configurações do Codex Minibar",
            ),
            (
                Language::Spanish,
                "Configuración",
                "Configuración de Codex Minibar",
            ),
            (Language::SimplifiedChinese, "设置", "Codex Minibar 设置"),
            (Language::TraditionalChinese, "設定", "Codex Minibar 設定"),
            (Language::Japanese, "設定", "Codex Minibar の設定"),
            (Language::Korean, "설정", "Codex Minibar 설정"),
            (
                Language::German,
                "Einstellungen",
                "Codex Minibar – Einstellungen",
            ),
            (
                Language::French,
                "Paramètres",
                "Paramètres de Codex Minibar",
            ),
        ] {
            assert_eq!(tr_in(language, "settings"), label);
            assert_eq!(tr_in(language, "codex-minibar-settings"), title);
            assert_eq!(tr_in(language, "unknown-key"), "unknown-key");
        }
        assert_eq!(english("settings"), "Settings");
    }

    #[test]
    fn new_locales_pluralize_numeric_string_parameters_and_preserve_account_data() {
        for (language, one, two, deleted) in [
            (
                Language::Korean,
                "API 키 1개",
                "API 키 2개",
                "Settings 삭제됨.",
            ),
            (
                Language::BrazilianPortuguese,
                "1 chave de API",
                "2 chaves de API",
                "Settings excluído.",
            ),
            (
                Language::Spanish,
                "1 clave de API",
                "2 claves de API",
                "Se eliminó Settings.",
            ),
            (
                Language::SimplifiedChinese,
                "1 个 API 密钥",
                "2 个 API 密钥",
                "已删除 Settings。",
            ),
            (
                Language::TraditionalChinese,
                "1 個 API 金鑰",
                "2 個 API 金鑰",
                "已刪除 Settings。",
            ),
            (
                Language::Japanese,
                "1 個の API キー",
                "2 個の API キー",
                "Settings を削除しました。",
            ),
            (
                Language::German,
                "1 API-Schlüssel",
                "2 API-Schlüssel",
                "Settings gelöscht.",
            ),
            (
                Language::French,
                "1 clé API",
                "2 clés API",
                "Settings supprimé.",
            ),
        ] {
            assert_eq!(
                format_in(language, "api-key-count", &[("v0", "1".into())]),
                one
            );
            assert_eq!(
                format_in(language, "api-key-count", &[("v0", "2".into())]),
                two
            );
            assert_eq!(
                format_in(language, "name-deleted", &[("name", "Settings".into())]),
                deleted
            );
        }
        for (language, zero) in [
            (Language::BrazilianPortuguese, "0 solicitação"),
            (Language::Spanish, "0 solicitudes"),
            (Language::SimplifiedChinese, "0 次请求"),
            (Language::TraditionalChinese, "0 次請求"),
            (Language::Japanese, "0 件のリクエスト"),
            (Language::German, "0 Anfragen"),
            (Language::French, "0 requête"),
            (Language::Korean, "요청 0건"),
        ] {
            assert_eq!(format_in(language, "requests", &[("v0", "0".into())]), zero);
        }
    }

    #[test]
    fn textual_dates_follow_locale_order_and_month_labels() {
        let date = chrono::NaiveDate::from_ymd_opt(2026, 10, 8).unwrap();
        for (language, expected) in [
            (Language::English, "Oct 8, 2026"),
            (Language::Russian, "8 окт., 2026"),
            (Language::BrazilianPortuguese, "8 de out de 2026"),
            (Language::Spanish, "8 de oct de 2026"),
            (Language::SimplifiedChinese, "2026年10月8日"),
            (Language::TraditionalChinese, "2026年10月8日"),
            (Language::Japanese, "2026年10月8日"),
            (Language::German, "8. Okt. 2026"),
            (Language::French, "8 oct. 2026"),
            (Language::Korean, "2026년 10월 8일"),
        ] {
            assert_eq!(date_with_year_in(language, date), expected);
        }
    }

    #[test]
    fn japanese_german_and_french_counts_follow_their_plural_rules() {
        for (number, german, french) in [
            (0, "0 Anfragen", "0 requête"),
            (1, "1 Anfrage", "1 requête"),
            (2, "2 Anfragen", "2 requêtes"),
            (1_000_000, "1000000 Anfragen", "1000000 requêtes"),
        ] {
            let values = [("v0", number.to_string())];
            assert_eq!(format_in(Language::German, "requests", &values), german);
            assert_eq!(format_in(Language::French, "requests", &values), french);
            assert_eq!(
                format_in(Language::Japanese, "requests", &values),
                format!("{number} 件のリクエスト")
            );
        }
        for (number, suffix) in [(0, "jour"), (1, "jour"), (2, "jours"), (1_000_000, "jours")] {
            assert_eq!(
                format_in(
                    Language::French,
                    "name-login-expires-in-days-left",
                    &[
                        ("name", "仕事 / Work".into()),
                        ("days_left", number.to_string())
                    ],
                ),
                format!("La connexion de 仕事 / Work expire dans {number} {suffix}")
            );
        }
    }

    #[test]
    fn japanese_dates_use_numeric_months_throughout_the_year() {
        for month in 1..=12 {
            let date = chrono::NaiveDate::from_ymd_opt(2026, month, 8).unwrap();
            assert_eq!(
                date_with_year_in(Language::Japanese, date),
                format!("2026年{month}月8日")
            );
        }
    }

    #[test]
    fn korean_dates_and_counts_preserve_parameters() {
        for month in 1..=12 {
            let date = chrono::NaiveDate::from_ymd_opt(2026, month, 8).unwrap();
            assert_eq!(
                date_with_year_in(Language::Korean, date),
                format!("2026년 {month}월 8일")
            );
        }
        for number in [0, 1, 2, 5, 21, 1_000_000] {
            assert_eq!(
                format_in(Language::Korean, "requests", &[("v0", number.to_string())]),
                format!("요청 {number}건")
            );
            assert_eq!(
                format_in(
                    Language::Korean,
                    "name-login-expires-in-days-left",
                    &[
                        ("name", "Work / 작업".into()),
                        ("days_left", number.to_string())
                    ]
                ),
                format!("Work / 작업 로그인은 {number}일 후 만료됩니다")
            );
        }
        assert_eq!(Language::labels()[Language::Korean.index()], "한국어");
        assert!(
            toml::to_string(&crate::settings::Settings {
                language: Language::Korean,
                ..Default::default()
            })
            .unwrap()
            .contains("language = \"korean\"")
        );
    }
}
