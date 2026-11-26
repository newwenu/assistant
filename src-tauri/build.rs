// 参考: feature-mount-traversal-spec.md（已归档） §4 S8 — build.rs 从插件真源派生命令清单

use std::fs;
use std::path::Path;

const SRC_DIR: &str = "src";
const PLUGIN_DIR: &str = "src/feature_plugins";
const PLUGIN_SUFFIX: &str = "_plugin.rs";
const HANDLER_OPEN: &str = "generate_handler![";
const COMMAND_PREFIX_TEMPLATE: &str = "crate::cmd::{}::";

fn main() {
    println!("cargo:rerun-if-changed={}", SRC_DIR);
    println!("cargo:rerun-if-changed={}", PLUGIN_DIR);

    let mut attributes = tauri_build::Attributes::new();
    for id in plugin_ids() {
        println!(
            "cargo:rerun-if-changed={}/{}{}",
            PLUGIN_DIR, id, PLUGIN_SUFFIX
        );

        let commands: &'static [&'static str] = Box::leak(
            plugin_commands(&id)
                .into_iter()
                .map(|name| Box::leak(name.into_boxed_str()) as &'static str)
                .collect::<Vec<&'static str>>()
                .into_boxed_slice(),
        );

        let plugin_name: &'static str = acl_plugin_name(&id);
        attributes = attributes.plugin(
            plugin_name,
            tauri_build::InlinedPlugin::new()
                .commands(commands)
                .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
        );
    }

    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}

/// ACL 标识符只允许小写 ASCII 与连字符，下划线非法（`tauri-utils` 的 `Identifier` 校验），
/// 所以插件名里的 `_` 一律转 `-`：运行时 `plugin::Builder::new()`、`capabilities/*.json`
/// 的权限前缀必须用同一个连字符名，而模块路径与文件名仍用下划线。
fn acl_plugin_name(id: &str) -> &'static str {
    Box::leak(id.replace('_', "-").into_boxed_str())
}

fn plugin_ids() -> Vec<String> {
    let entries = fs::read_dir(PLUGIN_DIR).expect("插件目录可读");
    let mut ids: Vec<String> = entries
        .map(|entry| entry.expect("源码条目可读").path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .filter_map(|path| {
            path.file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.strip_suffix("_plugin"))
                .map(|id| id.to_string())
        })
        .collect();
    ids.sort();
    ids
}

fn plugin_commands(id: &str) -> Vec<String> {
    let relative = format!("{}/{}{}", PLUGIN_DIR, id, PLUGIN_SUFFIX);
    let prefix = COMMAND_PREFIX_TEMPLATE.replace("{}", id);
    let text = fs::read_to_string(Path::new(&relative))
        .unwrap_or_else(|e| panic!("无法读取命令真源 {}: {}", relative, e));
    if text.matches(HANDLER_OPEN).count() != 1 {
        panic!(
            "{} 必须恰有 1 个 {} 注册块：多块会让后续命令被静默丢弃",
            relative, HANDLER_OPEN
        );
    }
    let open = text
        .find(HANDLER_OPEN)
        .unwrap_or_else(|| panic!("{} 缺少 {} 注册块", relative, HANDLER_OPEN));
    let body_start = open + HANDLER_OPEN.len();
    let body = &text[body_start..];
    let close = body
        .find(']')
        .unwrap_or_else(|| panic!("{} 的 {} 注册块未闭合", relative, HANDLER_OPEN));

    let mut commands: Vec<String> = Vec::new();
    for entry in body[..close].split(',') {
        let entry = entry.trim();
        let Some(rest) = entry.strip_prefix(&prefix) else {
            continue;
        };
        let name: String = rest
            .chars()
            .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
            .collect();
        if name.is_empty() {
            panic!("{} 出现无法解析的注册条目: {:?}", relative, entry);
        }
        if !commands.iter().any(|existing| existing == &name) {
            commands.push(name);
        }
    }

    if commands.is_empty() {
        panic!(
            "{} 未解析到任何命令，派生规则可能已失效（前缀 {}）",
            relative, prefix
        );
    }

    commands
}
