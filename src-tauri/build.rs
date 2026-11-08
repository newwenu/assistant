// 参考: docs/store/I24-feature-extension-spec.md §11.2.1 — build.rs 从插件真源派生命令清单

use std::fs;

const PLUGIN_SOURCE: &str = "src/vault_plugin.rs";
const HANDLER_OPEN: &str = "generate_handler![";
const COMMAND_PREFIX: &str = "crate::cmd::vault::";

fn main() {
    println!("cargo:rerun-if-changed={}", PLUGIN_SOURCE);

    let commands: &'static [&'static str] = Box::leak(
        plugin_commands(PLUGIN_SOURCE)
            .into_iter()
            .map(|name| Box::leak(name.into_boxed_str()) as &'static str)
            .collect::<Vec<&'static str>>()
            .into_boxed_slice(),
    );

    tauri_build::try_build(
        tauri_build::Attributes::new().plugin(
            "vault",
            tauri_build::InlinedPlugin::new()
                .commands(commands)
                .default_permission(tauri_build::DefaultPermissionRule::AllowAllCommands),
        ),
    )
    .expect("failed to run tauri-build");
}

fn plugin_commands(relative: &str) -> Vec<String> {
    let text = fs::read_to_string(relative)
        .unwrap_or_else(|e| panic!("无法读取命令真源 {}: {}", relative, e));
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
        let Some(rest) = entry.strip_prefix(COMMAND_PREFIX) else {
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
            relative, COMMAND_PREFIX
        );
    }

    commands
}
