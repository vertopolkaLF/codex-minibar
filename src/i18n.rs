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
const PT_BR: &str = include_str!("../locales/pt-BR/app.ftl");
const ES: &str = include_str!("../locales/es/app.ftl");
const ZH_CN: &str = include_str!("../locales/zh-CN/app.ftl");
const ZH_TW: &str = include_str!("../locales/zh-TW/app.ftl");
const JA: &str = include_str!("../locales/ja/app.ftl");
const DE: &str = include_str!("../locales/de/app.ftl");
const FR: &str = include_str!("../locales/fr/app.ftl");
const TR: &str = include_str!("../locales/tr/app.ftl");
const KO_KR: &str = include_str!("../locales/ko-KR/app.ftl");

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

static CURRENT: AtomicU8 = AtomicU8::new(Language::English.index() as u8);

impl Language {
    // Native names in alphabetical (Unicode) order, shared by every selector.
    pub const SUPPORTED: [Self; 11] = [
        Self::German,
        Self::English,
        Self::Spanish,
        Self::French,
        Self::BrazilianPortuguese,
        Self::Turkish,
        Self::Russian,
        Self::Japanese,
        Self::SimplifiedChinese,
        Self::TraditionalChinese,
        Self::Korean,
    ];

    pub fn apply(self) {
        CURRENT.store(self.resolved().index() as u8, Ordering::Release);
    }
    pub fn resolved(self) -> Self {
        if self == Self::Auto {
            system_language()
        } else {
            self
        }
    }
    pub const fn index(self) -> usize {
        match self {
            Self::Auto => 0,
            Self::German => 1,
            Self::English => 2,
            Self::Spanish => 3,
            Self::French => 4,
            Self::BrazilianPortuguese => 5,
            Self::Turkish => 6,
            Self::Russian => 7,
            Self::Japanese => 8,
            Self::SimplifiedChinese => 9,
            Self::TraditionalChinese => 10,
            Self::Korean => 11,
        }
    }
    pub fn from_index(index: usize) -> Self {
        Self::SUPPORTED
            .get(index.wrapping_sub(1))
            .copied()
            .unwrap_or(Self::Auto)
    }
    pub fn labels() -> [&'static str; 12] {
        [
            tr("auto-windows"),
            "Deutsch",
            "English",
            "Español",
            "Français",
            "Português (Brasil)",
            "Türkçe",
            "Русский",
            "日本語",
            "简体中文",
            "繁體中文",
            "한국어",
        ]
    }
}

pub fn current_language() -> Language {
    Language::from_index(CURRENT.load(Ordering::Acquire) as usize)
}

fn language_from_tag(tag: &str) -> Language {
    let normalized = tag.to_ascii_lowercase().replace('_', "-");
    let mut subtags = normalized.split(['-', '.', '@']);
    if subtags.next() == Some("zh") {
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
    match tag
        .split(['-', '_', '.', '@'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "ru" => Language::Russian,
        "pt" => Language::BrazilianPortuguese,
        "es" => Language::Spanish,
        "zh" => Language::SimplifiedChinese,
        "ja" => Language::Japanese,
        "de" => Language::German,
        "fr" => Language::French,
        "ko" => Language::Korean,
        "tr" => Language::Turkish,
        _ => Language::English,
    }
}

#[cfg(any(windows, test))]
fn language_from_windows_langid(langid: u16) -> Language {
    if matches!(langid, 0x0404 | 0x0c04 | 0x1404 | 0x7c04) {
        return Language::TraditionalChinese;
    }
    language_from_tag(match langid & 0x03ff {
        0x19 => "ru",
        0x16 => "pt",
        0x0a => "es",
        0x04 => "zh",
        0x11 => "ja",
        0x07 => "de",
        0x0c => "fr",
        0x12 => "ko",
        0x1f => "tr",
        _ => "en",
    })
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
        unix_system_language(|name| std::env::var(name).ok())
    }
}

#[cfg(any(not(windows), test))]
fn unix_system_language(mut read: impl FnMut(&str) -> Option<String>) -> Language {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|name| read(name).filter(|value| !value.is_empty()))
        .unwrap_or_else(|| "C".into());
    if matches!(locale.as_str(), "C" | "POSIX") {
        return Language::English;
    }
    if let Some(preferences) = read("LANGUAGE") {
        for tag in preferences.split(':').filter(|tag| !tag.is_empty()) {
            if matches!(tag, "C" | "POSIX") {
                return Language::English;
            }
            let language = language_from_tag(tag);
            let is_english = tag
                .split(['-', '_', '.', '@'])
                .next()
                .is_some_and(|base| base.eq_ignore_ascii_case("en"));
            if language != Language::English || is_english {
                return language;
            }
        }
    }
    language_from_tag(&locale)
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

struct Catalogs {
    en: FluentBundle<FluentResource>,
    ru: FluentBundle<FluentResource>,
    pt_br: FluentBundle<FluentResource>,
    es: FluentBundle<FluentResource>,
    zh_cn: FluentBundle<FluentResource>,
    zh_tw: FluentBundle<FluentResource>,
    ja: FluentBundle<FluentResource>,
    de: FluentBundle<FluentResource>,
    fr: FluentBundle<FluentResource>,
    ko_kr: FluentBundle<FluentResource>,
    tr: FluentBundle<FluentResource>,
}
impl Catalogs {
    fn new() -> Self {
        Self {
            en: bundle("en", EN),
            ru: bundle("ru", RU),
            pt_br: bundle("pt-BR", PT_BR),
            es: bundle("es", ES),
            zh_cn: bundle("zh-CN", ZH_CN),
            zh_tw: bundle("zh-TW", ZH_TW),
            ja: bundle("ja", JA),
            de: bundle("de", DE),
            fr: bundle("fr", FR),
            ko_kr: bundle("ko-KR", KO_KR),
            tr: bundle("tr", TR),
        }
    }
    fn for_language(&self, language: Language) -> &FluentBundle<FluentResource> {
        match language.resolved() {
            Language::Russian => &self.ru,
            Language::BrazilianPortuguese => &self.pt_br,
            Language::Spanish => &self.es,
            Language::SimplifiedChinese => &self.zh_cn,
            Language::TraditionalChinese => &self.zh_tw,
            Language::Japanese => &self.ja,
            Language::German => &self.de,
            Language::French => &self.fr,
            Language::Korean => &self.ko_kr,
            Language::Turkish => &self.tr,
            _ => &self.en,
        }
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
        resolve(self.for_language(language)).or_else(|| resolve(&self.en))
    }
}

thread_local! { static CATALOGS: RefCell<Catalogs> = RefCell::new(Catalogs::new()); }

static LABELS: LazyLock<HashMap<String, [&'static str; Language::SUPPORTED.len()]>> =
    LazyLock::new(|| {
        let catalogs = Catalogs::new();
        let mut labels = HashMap::new();
        for line in EN
            .lines()
            .filter(|line| !line.starts_with([' ', '#']) && line.contains(" = "))
        {
            let key = line.split(" = ").next().unwrap();
            if catalogs.resolve(Language::English, key, None).is_some() {
                let values = Language::SUPPORTED.map(|language| {
                    Box::leak(
                        catalogs
                            .resolve(language, key, None)
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
    LABELS
        .get(key)
        .map(|labels| labels[Language::English.index() - 1])
        .unwrap_or(key)
}

/// Resolve an explicit language without changing the process language. Also
/// used by a second process to find windows opened in any supported language.
pub fn tr_in(language: Language, key: &'static str) -> &'static str {
    LABELS
        .get(key)
        .map(|labels| labels[language.resolved().index() - 1])
        .unwrap_or(key)
}

/// This boundary is only for application error summaries, never user labels.
pub fn localize_error(source: &str) -> String {
    let index = current_language().index() - 1;
    source
        .lines()
        .map(|line| {
            LABELS
                .values()
                .find(|labels| labels[Language::English.index() - 1] == line)
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
        for (offset, language) in Language::SUPPORTED.into_iter().enumerate() {
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
        for (language, source) in [
            (Language::Russian, RU),
            (Language::BrazilianPortuguese, PT_BR),
            (Language::Spanish, ES),
            (Language::SimplifiedChinese, ZH_CN),
            (Language::TraditionalChinese, ZH_TW),
            (Language::Japanese, JA),
            (Language::German, DE),
            (Language::French, FR),
            (Language::Korean, KO_KR),
            (Language::Turkish, TR),
        ] {
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
        for language in Language::SUPPORTED {
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
        for language in std::iter::once(Language::Auto).chain(Language::SUPPORTED) {
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
            en: bundle(
                "en",
                "missing = English\nempty = English\nbad = Hello { $name }\n",
            ),
            ru: bundle("ru", "empty = { \"\" }\nbad = Привет { $unknown }\n"),
            pt_br: bundle("pt-BR", "empty = { \"\" }\nbad = Hello { $unknown }\n"),
            es: bundle("es", "empty = { \"\" }\nbad = Hello { $unknown }\n"),
            zh_cn: bundle("zh-CN", "empty = { \"\" }\nbad = Hello { $unknown }\n"),
            zh_tw: bundle("zh-TW", "empty = { \"\" }\nbad = Hello { $unknown }\n"),
            ja: bundle("ja", "empty = { \"\" }\nbad = Hello { $unknown }\n"),
            de: bundle("de", "empty = { \"\" }\nbad = Hello { $unknown }\n"),
            fr: bundle("fr", "empty = { \"\" }\nbad = Hello { $unknown }\n"),
            ko_kr: bundle("ko-KR", "empty = { \"\" }\nbad = Hello { $unknown }\n"),
            tr: bundle("tr", "empty = { \"\" }\nbad = Hello { $unknown }\n"),
        };
        let mut args = FluentArgs::new();
        args.set("name", "Ada");
        for language in Language::SUPPORTED
            .into_iter()
            .filter(|language| *language != Language::English)
        {
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
    fn unix_ui_language_honors_priority_lists_and_message_locale_precedence() {
        for (all, messages, lang, preferences, expected) in [
            (None, None, "en_US.UTF-8", "sv:de", Language::German),
            (None, None, "ru_RU.UTF-8", "en:ru", Language::English),
            (
                None,
                None,
                "en_US.UTF-8",
                "xx:zh_TW:de",
                Language::TraditionalChinese,
            ),
            (Some("C"), Some("ru_RU"), "de_DE", "ru", Language::English),
            (None, Some("POSIX"), "de_DE", "ru", Language::English),
            (None, Some("fr_FR.UTF-8"), "de_DE", "xx", Language::French),
            (
                Some("ru_RU.UTF-8"),
                Some("fr_FR"),
                "de_DE",
                "",
                Language::Russian,
            ),
        ] {
            assert_eq!(
                unix_system_language(|name| match name {
                    "LC_ALL" => all.map(str::to_owned),
                    "LC_MESSAGES" => messages.map(str::to_owned),
                    "LANG" => Some(lang.into()),
                    "LANGUAGE" => Some(preferences.into()),
                    _ => None,
                }),
                expected
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
        for language in std::iter::once(Language::Auto).chain(Language::SUPPORTED) {
            assert_eq!(Language::from_index(language.index()), language);
        }
        assert_eq!(Language::from_index(usize::MAX), Language::Auto);
        assert_eq!(Language::labels().len(), Language::SUPPORTED.len() + 1);
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

    #[test]
    fn every_locale_directory_has_the_complete_english_key_set() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("locales");
        let english = catalog_parameters(EN);
        let expected = english.keys().collect::<BTreeSet<_>>();
        let mut count = 0;
        for entry in std::fs::read_dir(root).unwrap() {
            let entry = entry.unwrap();
            if !entry.file_type().unwrap().is_dir() {
                continue;
            }
            let path = entry.path().join("app.ftl");
            let source = std::fs::read_to_string(&path).unwrap();
            let messages = catalog_parameters(&source);
            assert_eq!(
                messages.keys().collect::<BTreeSet<_>>(),
                expected,
                "{}",
                path.display()
            );
            count += 1;
        }
        assert!(
            count > Language::SUPPORTED.len(),
            "include English and the template"
        );
    }
}
