use log::error;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::RwLock;

static ZH_CN: &str = include_str!("../locales/zh-CN.json");
static EN_US: &str = include_str!("../locales/en-US.json");

static GLOBAL_I18N: RwLock<Option<I18n>> = RwLock::new(None);

pub struct I18n {
    locale: String,
    flat: HashMap<String, String>,
}

impl I18n {
    pub fn new(locale: &str) -> Self {
        let flat = Self::load_and_flatten(locale);
        Self {
            locale: locale.to_string(),
            flat,
        }
    }

    fn load_and_flatten(locale: &str) -> HashMap<String, String> {
        let json_str = match locale {
            "zh-CN" => ZH_CN,
            "en-US" => EN_US,
            _ => ZH_CN,
        };

        let mut flat = HashMap::new();
        match serde_json::from_str::<Value>(json_str) {
            Ok(root) => Self::flatten_value(&root, String::new(), &mut flat),
            Err(e) => {
                error!("[i18n] Failed to parse locale {}: {}", locale, e);
            }
        }
        flat
    }

    fn flatten_value(value: &Value, prefix: String, out: &mut HashMap<String, String>) {
        match value {
            Value::Object(obj) => {
                for (k, v) in obj {
                    let new_prefix = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{}.{}", prefix, k)
                    };
                    Self::flatten_value(v, new_prefix, out);
                }
            }
            Value::String(s) => {
                out.insert(prefix, s.clone());
            }
            Value::Number(n) => {
                out.insert(prefix, n.to_string());
            }
            Value::Bool(b) => {
                out.insert(prefix, b.to_string());
            }
            Value::Null => {
                out.insert(prefix, String::new());
            }
            _ => {}
        }
    }

    pub fn t(&self, key: &str) -> String {
        self.flat
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    pub fn t_with_args(&self, key: &str, args: &HashMap<&str, &str>) -> String {
        let template = self
            .flat
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_string());
        let mut result = template;
        for (k, v) in args {
            let placeholder = format!("{{{{{}}}}}", k);
            result = result.replace(&placeholder, v);
        }
        result
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }
}

pub fn init_global(locale: &str) {
    let i18n = I18n::new(locale);
    if let Ok(mut guard) = GLOBAL_I18N.write() {
        *guard = Some(i18n);
    }
}

pub fn set_locale(locale: &str) {
    let i18n = I18n::new(locale);
    if let Ok(mut guard) = GLOBAL_I18N.write() {
        *guard = Some(i18n);
    }
}

pub fn t(key: &str) -> String {
    if let Ok(guard) = GLOBAL_I18N.read() {
        if let Some(i18n) = guard.as_ref() {
            return i18n.t(key);
        }
    }
    key.to_string()
}

pub fn t_with_args(key: &str, args: &HashMap<&str, &str>) -> String {
    if let Ok(guard) = GLOBAL_I18N.read() {
        if let Some(i18n) = guard.as_ref() {
            return i18n.t_with_args(key, args);
        }
    }
    key.to_string()
}

pub fn current_locale() -> String {
    if let Ok(guard) = GLOBAL_I18N.read() {
        if let Some(i18n) = guard.as_ref() {
            return i18n.locale().to_string();
        }
    }
    "zh-CN".to_string()
}
