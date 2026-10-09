// 参考: BongoCat/crates/bongocat-i18n — i18n 加载逻辑

use log::error;
use serde_json::Value;
use std::collections::HashMap;

static ZH_CN: &str = include_str!("../locales/zh-CN.json");
static EN_US: &str = include_str!("../locales/en-US.json");

pub struct I18n {
    locale: String,
    translations: HashMap<String, Value>,
}

impl I18n {
    pub fn new(locale: &str) -> Self {
        let translations = Self::load_translations(locale);
        Self {
            locale: locale.to_string(),
            translations,
        }
    }

    fn load_translations(locale: &str) -> HashMap<String, Value> {
        let json_str = match locale {
            "zh-CN" => ZH_CN,
            "en-US" => EN_US,
            _ => ZH_CN,
        };

        let mut map = HashMap::new();
        match serde_json::from_str::<Value>(json_str) {
            Ok(Value::Object(obj)) => {
                for (k, v) in obj {
                    map.insert(k, v);
                }
            }
            Err(e) => {
                error!("[i18n] Failed to parse locale {}: {}", locale, e);
            }
            _ => {}
        }
        map
    }

    pub fn t(&self, key: &str) -> String {
        if let Some(Value::String(s)) = self.translations.get(key) {
            return s.clone();
        }
        key.to_string()
    }

    pub fn locale(&self) -> &str {
        &self.locale
    }
}
