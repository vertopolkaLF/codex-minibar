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
}

static CURRENT: AtomicU8 = AtomicU8::new(1);

impl Language {
    pub const SUPPORTED: [Self; 5] = [
        Self::English,
        Self::Russian,
        Self::BrazilianPortuguese,
        Self::Spanish,
        Self::SimplifiedChinese,
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
    pub fn index(self) -> usize {
        match self {
            Self::Auto => 0,
            Self::English => 1,
            Self::Russian => 2,
            Self::BrazilianPortuguese => 3,
            Self::Spanish => 4,
            Self::SimplifiedChinese => 5,
        }
    }
    pub fn from_index(index: usize) -> Self {
        Self::SUPPORTED
            .get(index.wrapping_sub(1))
            .copied()
            .unwrap_or(Self::Auto)
    }
    pub fn labels() -> [&'static str; 6] {
        [
            tr("auto-windows"),
            "English",
            "Русский",
            "Português (Brasil)",
            "Español",
            "简体中文",
        ]
    }
}

pub fn current_language() -> Language {
    Language::from_index(CURRENT.load(Ordering::Acquire) as usize)
}

fn language_from_tag(tag: &str) -> Language {
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
        _ => Language::English,
    }
}

fn system_language() -> Language {
    #[cfg(windows)]
    {
        // UI language, not the regional date/number format or keyboard layout.
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GetUserDefaultUILanguage() -> u16;
        }
        language_from_tag(match unsafe { GetUserDefaultUILanguage() & 0x03ff } {
            0x19 => "ru",
            0x16 => "pt",
            0x0a => "es",
            0x04 => "zh",
            _ => "en",
        })
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

struct Catalogs {
    en: FluentBundle<FluentResource>,
    ru: FluentBundle<FluentResource>,
    pt_br: FluentBundle<FluentResource>,
    es: FluentBundle<FluentResource>,
    zh_cn: FluentBundle<FluentResource>,
}
impl Catalogs {
    fn new() -> Self {
        Self {
            en: bundle("en", EN),
            ru: bundle("ru", RU),
            pt_br: bundle("pt-BR", PT_BR),
            es: bundle("es", ES),
            zh_cn: bundle("zh-CN", ZH_CN),
        }
    }
    fn for_language(&self, language: Language) -> &FluentBundle<FluentResource> {
        match language.resolved() {
            Language::Russian => &self.ru,
            Language::BrazilianPortuguese => &self.pt_br,
            Language::Spanish => &self.es,
            Language::SimplifiedChinese => &self.zh_cn,
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

static LABELS: LazyLock<HashMap<String, [&'static str; 5]>> = LazyLock::new(|| {
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
    LABELS.get(key).map(|labels| labels[0]).unwrap_or(key)
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
                .find(|labels| labels[0] == line)
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
        Language::Russian => format!("{} {month}", date.day()),
        Language::BrazilianPortuguese | Language::Spanish => format!("{} de {month}", date.day()),
        Language::SimplifiedChinese => format!("{month}{}日", date.day()),
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
        Language::SimplifiedChinese => format!("{}年{}", date.year(), month_day_in(language, date)),
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
        };
        let mut args = FluentArgs::new();
        args.set("name", "Ada");
        for language in Language::SUPPORTED.into_iter().skip(1) {
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
            ("ru_RU", Language::Russian),
            ("de-DE", Language::English),
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
        ] {
            assert_eq!(date_with_year_in(language, date), expected);
        }
    }
}
