use std::{
    collections::HashMap,
    sync::{LazyLock, OnceLock, RwLock},
};

use serde_json::Value;

#[doc(hidden)]
pub const CATALOG_REVISION: &str = env!("ASSISTANT_I18N_CATALOG_REVISION");

pub const DEFAULT_LOCALE: &str = "zh-CN";

pub const SHIPPED_LOCALES: [&str; 2] = ["zh-CN", "en-US"];

static ZH_CN: &str = include_str!("../locales/zh-CN.json");
static EN_US: &str = include_str!("../locales/en-US.json");

fn flatten_json(value: &Value, prefix: &str, out: &mut HashMap<String, String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let new_prefix = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                flatten_json(v, &new_prefix, out);
            }
        }
        Value::String(s) => {
            out.insert(prefix.to_string(), s.clone());
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::Array(_) => {
            out.insert(prefix.to_string(), value.to_string());
        }
    }
}

fn load_catalog(raw: &str) -> HashMap<String, String> {
    let parsed: Value = serde_json::from_str(raw).expect("parse locale JSON");
    let mut flat = HashMap::new();
    flatten_json(&parsed, "", &mut flat);
    flat
}

struct Catalog {
    zh_cn: HashMap<String, String>,
    en_us: HashMap<String, String>,
}

static CATALOG: OnceLock<Catalog> = OnceLock::new();

fn catalog() -> &'static Catalog {
    CATALOG.get_or_init(|| Catalog {
        zh_cn: load_catalog(ZH_CN),
        en_us: load_catalog(EN_US),
    })
}

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
    let resolved = locale_code(locale);
    let cache_key = format!("{resolved}\0{key}");
    if let Some(value) = cache
        .read()
        .expect("translation cache read lock")
        .get(&cache_key)
    {
        return value;
    }
    let cat = catalog();
    let value = match resolved {
        "zh-CN" => cat.zh_cn.get(key),
        "en-US" => cat.en_us.get(key),
        _ => cat.zh_cn.get(key),
    };
    let raw = value.map(|s| s.as_str()).unwrap_or(key);
    let leaked: &'static str = Box::leak(raw.to_string().into_boxed_str());
    cache
        .write()
        .expect("translation cache write lock")
        .insert(cache_key, leaked);
    leaked
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
