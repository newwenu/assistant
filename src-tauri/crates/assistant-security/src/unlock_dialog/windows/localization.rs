#[link(name = "kernel32")]
extern "system" {
    fn GetUserDefaultUILanguage() -> u16;
}

pub(super) struct UiStrings {
    pub title: &'static str,
    pub label: &'static str,
    pub unlock: &'static str,
    pub close: &'static str,
}

fn is_chinese_ui() -> bool {
    let lang_id = unsafe { GetUserDefaultUILanguage() };
    let primary_lang = lang_id & 0x3F;
    primary_lang == 0x04
}

pub(super) fn get_ui_strings() -> UiStrings {
    if is_chinese_ui() {
        UiStrings {
            title: "保险库解锁",
            label: "请输入主密码：",
            unlock: "解锁",
            close: "关闭",
        }
    } else {
        UiStrings {
            title: "Vault Unlock",
            label: "Enter master password:",
            unlock: "Unlock",
            close: "Close",
        }
    }
}

pub(super) fn str_to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0u16)).collect()
}
