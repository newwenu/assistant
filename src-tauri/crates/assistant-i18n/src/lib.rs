use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock, RwLock},
};

#[macro_use]
extern crate rust_i18n;

rust_i18n::i18n!("locales", fallback = "zh-CN");

#[doc(hidden)]
pub const CATALOG_REVISION: &str = env!("ASSISTANT_I18N_CATALOG_REVISION");

pub const DEFAULT_LOCALE: &str = "zh-CN";

pub const SHIPPED_LOCALES: [&str; 2] = ["zh-CN", "en-US"];

pub fn locale_code(code: &str) -> &str {
    let normalized = code.replace('_', "-").to_ascii_lowercase();
    SHIPPED_LOCALES
        .iter()
        .copied()
        .find(|shipped| shipped.to_ascii_lowercase() == normalized)
        .unwrap_or(DEFAULT_LOCALE)
}

pub fn text(locale: &str, key: &str) -> &'static str {
    static CACHE: OnceLock<RwLock<HashMap<String, &'static str>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| RwLock::new(HashMap::new()));
    let cache_key = format!("{}\0{key}", locale_code(locale));
    if let Some(value) = cache
        .read()
        .expect("translation cache read lock")
        .get(&cache_key)
    {
        return value;
    }
    let value = Box::leak(
        t!(key, locale = locale_code(locale))
            .to_string()
            .into_boxed_str(),
    );
    cache
        .write()
        .expect("translation cache write lock")
        .insert(cache_key, value);
    value
}

pub fn format_text(locale: &str, key: &str, values: &[(&str, String)]) -> String {
    let mut message = text(locale, key).to_owned();
    for (name, value) in values {
        message = message.replace(&format!("%{{{name}}}"), value);
    }
    message
}

static CURRENT_LOCALE: LazyLock<RwLock<String>> =
    LazyLock::new(|| RwLock::new(String::from(DEFAULT_LOCALE)));

pub fn init_global(locale: &str) {
    let resolved = locale_code(locale);
    if let Ok(mut guard) = CURRENT_LOCALE.write() {
        *guard = resolved.to_string();
    }
}

pub fn set_locale(locale: &str) {
    let resolved = locale_code(locale);
    if let Ok(mut guard) = CURRENT_LOCALE.write() {
        *guard = resolved.to_string();
    }
}

pub fn t(key: &str) -> String {
    let locale = current_locale();
    text(&locale, key).to_string()
}

pub fn t_with_args(key: &str, args: &HashMap<&str, &str>) -> String {
    let locale = current_locale();
    format_text(
        &locale,
        key,
        &args
            .iter()
            .map(|(k, v)| (*k, v.to_string()))
            .collect::<Vec<_>>(),
    )
}

pub fn current_locale() -> String {
    CURRENT_LOCALE
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| DEFAULT_LOCALE.to_string())
}
