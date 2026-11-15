// 参考: docs/store/feature-mount-traversal-spec.md §6 — 装配契约 A1–A8 / B1–B9 断言

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

const REGISTRATION_OPEN: &str = "generate_handler![";
const CFG_PREFIX: &str = "#[cfg(";
const CFG_FEATURE_MARKER: &str = "feature = \"";
const CFG_ALLOWED_IN: &str = "cmd/mod.rs";
const FEATURE_CFG_LIMIT: usize = 3;
const LIB_RS_LINE_LIMIT: usize = 40;
const ACL_ALLOW_PREFIX: &str = "allow-";
const ACL_PARSER_FLOOR: usize = 60;
const CONF_APPLIER_OPEN: &str = "register_conf_applier(";
const CONFIG_CONF_MODULE_SUFFIX: &str = "_conf";
const CONF_GROUP_DECL: &str = "GROUP: &'static str = \"";
const MOUNT_ID_MARKER: &str = "id: \"";
const MOUNT_TABLE_TYPE: &str = "&[FeatureMount]";
const MOUNT_ACCESSOR: &str = "fn mounts(";
const PLUGIN_FILE_SUFFIX: &str = "_plugin.rs";
const PLUGIN_DISCOVERY_MARKER: &str = "read_dir";
const CAPABILITIES_DIR: &str = "capabilities";
const FEATURE_MANIFEST: &str = "config/features.json";
const CARGO_MANIFEST: &str = "Cargo.toml";
const CALLER_FILES: [&str; 3] = ["plugins.rs", "lifecycle.rs", "setup.rs"];

fn manifest_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn src_root() -> PathBuf {
    manifest_dir().join("src")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = fs::read_dir(dir).expect("source directory is readable");
    for entry in entries {
        let path = entry.expect("source entry is readable").path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            out.push(path);
        }
    }
}

fn read(relative: &str) -> String {
    let path = src_root().join(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} is readable: {}", relative, e))
}

fn plugin_registry_files() -> Vec<PathBuf> {
    let mut files = Vec::new();
    let entries = fs::read_dir(src_root()).expect("source root is readable");
    for entry in entries {
        let path = entry.expect("source entry is readable").path();
        let is_rust = path.extension().and_then(|ext| ext.to_str()) == Some("rs");
        let is_plugin = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .map(|stem| stem.ends_with("_plugin"))
            .unwrap_or(false);
        if is_rust && is_plugin {
            files.push(path);
        }
    }
    files.sort();
    files
}

fn plugin_registry_names() -> Result<Vec<String>, String> {
    let mut names = Vec::new();
    for file in plugin_registry_files() {
        let text = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("{} is readable: {}", file.display(), e));
        names.extend(registered_names(&text)?);
    }
    Ok(names)
}

fn strip_rust_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' {
            match chars.peek() {
                Some('/') => {
                    for c in chars.by_ref() {
                        if c == '\n' {
                            out.push('\n');
                            break;
                        }
                    }
                    continue;
                }
                Some('*') => {
                    chars.next();
                    let mut prev = '\0';
                    for c in chars.by_ref() {
                        if prev == '*' && c == '/' {
                            break;
                        }
                        prev = c;
                    }
                    out.push(' ');
                    continue;
                }
                _ => {}
            }
        }
        out.push(ch);
    }
    out
}

fn registration_bodies(text: &str) -> Vec<String> {
    let mut bodies = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = text[cursor..].find(REGISTRATION_OPEN) {
        let start = cursor + offset + REGISTRATION_OPEN.len();
        let mut depth = 1usize;
        let mut close = None;
        for (index, ch) in text[start..].char_indices() {
            match ch {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(start + index);
                        break;
                    }
                }
                _ => {}
            }
        }
        let end = close.unwrap_or(text.len());
        bodies.push(text[start..end].to_string());
        if end >= text.len() {
            break;
        }
        cursor = end + 1;
    }
    bodies
}

fn is_valid_entry(entry: &str) -> bool {
    !entry.is_empty()
        && entry
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == ':')
}

fn registered_names(text: &str) -> Result<Vec<String>, String> {
    let cleaned = strip_rust_comments(text);
    let mut names = Vec::new();
    for body in registration_bodies(&cleaned) {
        for entry in body.split(',') {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                continue;
            }
            if !is_valid_entry(trimmed) {
                return Err(format!("注册表条目不是纯标识符路径: {:?}", trimmed));
            }
            let segment = trimmed.rsplit("::").next().unwrap_or(trimmed);
            names.push(segment.to_string());
        }
    }
    Ok(names)
}

fn string_literals(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '"' {
            continue;
        }
        let mut literal = String::new();
        for c in chars.by_ref() {
            if c == '"' {
                break;
            }
            literal.push(c);
        }
        out.push(literal);
    }
    out
}

fn command_to_permission(name: &str) -> String {
    format!("{}{}", ACL_ALLOW_PREFIX, name.replace('_', "-"))
}

fn permission_to_command(permission: &str) -> Option<String> {
    permission
        .strip_prefix(ACL_ALLOW_PREFIX)
        .map(|rest| rest.replace('-', "_"))
}

fn build_rs_text() -> String {
    let path = manifest_dir().join("build.rs");
    let text = fs::read_to_string(&path).expect("build.rs is readable");
    strip_rust_comments(&text)
}

fn manifest_feature_ids() -> Vec<String> {
    let mut ids: Vec<String> = manifest_feature_entries()
        .into_iter()
        .map(|(id, _)| id)
        .collect();
    ids.sort();
    ids
}

fn manifest_cargo_features() -> Vec<String> {
    let mut features: Vec<String> = manifest_feature_entries()
        .into_iter()
        .map(|(_, entry)| {
            entry
                .get("cargoFeature")
                .and_then(|value| value.as_str())
                .unwrap_or_else(|| panic!("features.json 条目缺少 cargoFeature"))
                .to_string()
        })
        .collect();
    features.sort();
    features
}

fn manifest_feature_entries() -> Vec<(String, serde_json::Value)> {
    let path = manifest_dir()
        .parent()
        .expect("crate lives under repo root")
        .join(FEATURE_MANIFEST);
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is readable: {}", path.display(), e));
    let value: serde_json::Value =
        serde_json::from_str(&text).expect("features.json is valid JSON");
    value
        .as_object()
        .expect("features.json is an object")
        .iter()
        .map(|(id, entry)| (id.clone(), entry.clone()))
        .collect()
}

fn cargo_feature_ids() -> Vec<String> {
    let path = manifest_dir().join(CARGO_MANIFEST);
    let text = fs::read_to_string(&path).expect("Cargo.toml is readable");
    let mut ids = Vec::new();
    let mut in_features = false;
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_features = trimmed == "[features]";
            continue;
        }
        if !in_features {
            continue;
        }
        let Some((key, _)) = trimmed.split_once('=') else {
            continue;
        };
        let key = key.trim();
        if key == "default" || key.is_empty() {
            continue;
        }
        ids.push(key.to_string());
    }
    ids.sort();
    ids
}

fn mounts_ids() -> Vec<String> {
    let text = read("feature_mounts.rs");
    let cleaned = strip_rust_comments(&text);
    let mut ids = Vec::new();
    let mut cursor = 0;
    while let Some(offset) = cleaned[cursor..].find(MOUNT_ID_MARKER) {
        let start = cursor + offset + MOUNT_ID_MARKER.len();
        let end = cleaned[start..]
            .find('"')
            .unwrap_or_else(|| panic!("feature_mounts.rs 的 id 字面量未闭合"));
        ids.push(cleaned[start..start + end].to_string());
        cursor = start + end;
    }
    ids.sort();
    ids
}

fn cfg_attr_count(text: &str, feature: &str) -> usize {
    cfg_feature_names(text)
        .into_iter()
        .filter(|name| name == feature)
        .count()
}

fn negated_block_marker(feature: &str) -> String {
    format!("#[cfg(not(feature = \"{}\"))]", feature)
}

fn enabled_block_marker(feature: &str) -> String {
    format!("#[cfg(feature = \"{}\")]", feature)
}

fn fn_signature(text: &str, file: &str, name: &str) -> String {
    let needle = format!("fn {}(", name);
    let start = text
        .find(&needle)
        .unwrap_or_else(|| panic!("{} 缺少 {}", file, needle));
    let body = &text[start + needle.len()..];
    let mut depth = 1usize;
    let mut close = None;
    for (index, ch) in body.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(index);
                    break;
                }
            }
            _ => {}
        }
    }
    let params = &body[..close.unwrap_or_else(|| panic!("{} 的参数表未闭合", file))];
    let tail = &body[close.unwrap() + 1..];

    let mut normalized = Vec::new();
    for param in params.split(',') {
        let param = param.trim();
        if param.is_empty() {
            continue;
        }
        let (name, ty) = param
            .split_once(':')
            .unwrap_or_else(|| panic!("{} 的参数缺少类型: {}", file, param));
        let name = name.trim().trim_start_matches('_');
        let ty = shorten_paths(ty).split_whitespace().collect::<String>();
        normalized.push(format!("{}:{}", name, ty));
    }

    let ret = match tail.split_once("->") {
        Some((_, ty)) => shorten_paths(ty.split('{').next().unwrap_or(""))
            .split_whitespace()
            .collect::<String>(),
        None => String::new(),
    };

    format!("({})->{}", normalized.join(","), ret)
}

fn plugin_feature_names() -> Vec<String> {
    let mut names: Vec<String> = plugin_registry_files()
        .iter()
        .filter_map(|file| {
            file.file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem| stem.strip_suffix("_plugin"))
                .map(|name| name.to_string())
        })
        .collect();
    names.sort();
    names
}

fn acl_command_names(feature: &str) -> Result<BTreeSet<String>, String> {
    let path = manifest_dir()
        .join("gen")
        .join("schemas")
        .join("acl-manifests.json");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| {
        panic!(
            "{} 不可读（先执行一次构建以生成 ACL）: {}",
            path.display(),
            e
        )
    });
    let value: serde_json::Value =
        serde_json::from_str(&text).expect("acl-manifests.json 是合法 JSON");
    let permissions = value
        .get(feature)
        .and_then(|v| v.get("permissions"))
        .and_then(|v| v.as_object())
        .ok_or_else(|| format!("acl-manifests.json 缺少 {} 的 permissions 对象", feature))?;

    let mut names = BTreeSet::new();
    for key in permissions.keys() {
        let Some(command) = permission_to_command(key) else {
            continue;
        };
        names.insert(command);
    }
    if names.is_empty() {
        return Err(format!(
            "acl-manifests.json 的 {} 未包含任何 {}* 权限",
            feature, ACL_ALLOW_PREFIX
        ));
    }
    Ok(names)
}

fn conf_applier_groups() -> Vec<String> {
    let mut files = vec![src_root().join("cmd").join("mod.rs")];
    files.extend(plugin_registry_files());
    let mut groups = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file)
            .unwrap_or_else(|e| panic!("{} is readable: {}", file.display(), e));
        let cleaned = strip_rust_comments(&text);
        let mut cursor = 0;
        while let Some(offset) = cleaned[cursor..].find(CONF_APPLIER_OPEN) {
            let after = cursor + offset + CONF_APPLIER_OPEN.len();
            let rest = cleaned[after..].trim_start();
            cursor = after;
            let Some(quoted) = rest.strip_prefix('"') else {
                continue;
            };
            let end = quoted.find('"').unwrap_or_else(|| {
                panic!(
                    "{} 的 register_conf_applier 分组字面量未闭合",
                    file.display()
                )
            });
            groups.push(quoted[..end].to_string());
        }
    }
    groups
}

fn config_conf_groups() -> Vec<String> {
    let path = manifest_dir()
        .join("crates")
        .join("assistant-config")
        .join("src")
        .join("lib.rs");
    let text = fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is readable: {}", path.display(), e));
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            let module = trimmed.strip_prefix("pub mod ")?.strip_suffix(';')?;
            module
                .strip_suffix(CONFIG_CONF_MODULE_SUFFIX)
                .map(|group| group.to_string())
        })
        .collect()
}

fn conf_group_literal(text: &str) -> Option<String> {
    let cleaned = strip_rust_comments(text);
    let idx = cleaned.find(CONF_GROUP_DECL)?;
    let rest = &cleaned[idx + CONF_GROUP_DECL.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn config_conf_file_groups() -> Vec<(String, String)> {
    let dir = manifest_dir()
        .join("crates")
        .join("assistant-config")
        .join("src");
    let mut out = Vec::new();
    let entries = fs::read_dir(&dir).expect("assistant-config/src 可读");
    for entry in entries {
        let path = entry.expect("assistant-config/src 目录项可读").path();
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Some(module) = stem.strip_suffix(CONFIG_CONF_MODULE_SUFFIX) else {
            continue;
        };
        let text =
            fs::read_to_string(&path).unwrap_or_else(|e| panic!("{} 可读: {}", path.display(), e));
        let group = conf_group_literal(&text)
            .unwrap_or_else(|| panic!("{} 缺少 {} 字面量", path.display(), CONF_GROUP_DECL));
        out.push((module.to_string(), group));
    }
    out.sort();
    out
}

fn feature_conf_groups() -> Vec<String> {
    let crates_dir = manifest_dir().join("crates");
    let mut out = Vec::new();
    let entries = fs::read_dir(&crates_dir).expect("crates 目录可读");
    for entry in entries {
        let conf = entry
            .expect("crates 目录项可读")
            .path()
            .join("src")
            .join("conf.rs");
        if !conf.is_file() {
            continue;
        }
        let text =
            fs::read_to_string(&conf).unwrap_or_else(|e| panic!("{} 可读: {}", conf.display(), e));
        let group = conf_group_literal(&text)
            .unwrap_or_else(|| panic!("{} 缺少 {} 字面量", conf.display(), CONF_GROUP_DECL));
        out.push(group);
    }
    out.sort();
    out
}

fn shorten_paths(text: &str) -> String {
    fn flush(token: &mut String, out: &mut String) {
        if token.is_empty() {
            return;
        }
        match token.rfind("::") {
            Some(idx) => out.push_str(&token[idx + 2..]),
            None => out.push_str(token),
        }
        token.clear();
    }

    let mut out = String::new();
    let mut token = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == ':' {
            token.push(ch);
        } else {
            flush(&mut token, &mut out);
            out.push(ch);
        }
    }
    flush(&mut token, &mut out);
    out
}

fn cfg_feature_names(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with(CFG_PREFIX) {
            continue;
        }
        if let Some(name) = extract_feature_name(trimmed) {
            names.push(name);
        }
    }
    names
}

fn extract_feature_name(line: &str) -> Option<String> {
    let start = line.find(CFG_FEATURE_MARKER)? + CFG_FEATURE_MARKER.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn cfg_location_allowed(feature: &str, relative: &str) -> bool {
    if relative == CFG_ALLOWED_IN {
        return true;
    }
    let expected = format!("{}_plugin.rs", feature.replace('-', "_"));
    relative.rsplit('/').next() == Some(expected.as_str())
}

#[test]
fn a1_registry_file_has_no_cfg() {
    let ipc = read("ipc.rs");
    assert!(
        !ipc.contains("#[cfg"),
        "ipc.rs 是纯核心注册表，不得出现任何 #[cfg]"
    );
}

#[test]
fn a2_feature_cfg_only_in_boundary_files() {
    let mut files = Vec::new();
    collect_rs_files(&src_root(), &mut files);

    let mut offenders = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("source file is readable");
        let relative = file
            .strip_prefix(src_root())
            .expect("file lives under src")
            .to_string_lossy()
            .replace('\\', "/");
        for feature in cfg_feature_names(&text) {
            if !cfg_location_allowed(&feature, &relative) {
                offenders.push(format!("{} -> feature \"{}\"", relative, feature));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "特性 #[cfg] 只能出现在 {} 或与特性同名的 *_plugin.rs，越界: {:?}",
        CFG_ALLOWED_IN,
        offenders
    );
}

#[test]
fn a3_lib_rs_is_assembly_only() {
    let lib = read("lib.rs");
    let lines = lib.lines().count();
    assert!(
        lines <= LIB_RS_LINE_LIMIT,
        "lib.rs 共 {} 行，超过装配层上限 {}",
        lines,
        LIB_RS_LINE_LIMIT
    );
    assert!(
        !lib.contains(REGISTRATION_OPEN),
        "lib.rs 不得内联 generate_handler! 注册表"
    );
}

#[test]
fn a4_plugin_registry_and_acl_grants_match_both_ways() {
    let mut acl_all = BTreeSet::new();
    let mut cross_feature = Vec::new();
    for feature in plugin_feature_names() {
        let acl = acl_command_names(&feature).unwrap_or_else(|e| panic!("{}", e));
        let repeated: Vec<String> = acl_all.intersection(&acl).cloned().collect();
        if !repeated.is_empty() {
            cross_feature.push(format!("{} 与其它特性重复: {:?}", feature, repeated));
        }
        acl_all.extend(acl);
    }

    let plugin: BTreeSet<String> = plugin_registry_names()
        .expect("插件注册表可解析")
        .into_iter()
        .collect();

    let missing_grant: Vec<&String> = plugin.difference(&acl_all).collect();
    let missing_registration: Vec<&String> = acl_all.difference(&plugin).collect();

    assert!(
        cross_feature.is_empty(),
        "同一命令被多个插件声明: {:?}",
        cross_feature
    );
    assert!(
        missing_grant.is_empty(),
        "已注册但未获得 ACL 授权: {:?}",
        missing_grant
    );
    assert!(
        missing_registration.is_empty(),
        "已获得 ACL 授权但未注册: {:?}",
        missing_registration
    );
}

#[test]
fn a5_parser_floor() {
    let problems: Vec<String> = plugin_feature_names()
        .into_iter()
        .filter_map(|feature| {
            let plugin_file = format!("{}{}", feature.replace('-', "_"), PLUGIN_FILE_SUFFIX);
            let names = registered_names(&read(&plugin_file))
                .unwrap_or_else(|e| panic!("{} 注册表可解析: {}", plugin_file, e));
            if names.is_empty() {
                Some(format!("{} 未解析到任何命令", plugin_file))
            } else {
                None
            }
        })
        .collect();

    assert!(
        problems.is_empty(),
        "每个 *_plugin.rs 至少要注册 1 条命令，解析器可能已失效: {:?}",
        problems
    );
}

#[test]
fn a6_registration_partition() {
    let core = registered_names(&read("ipc.rs")).expect("ipc.rs 注册表可解析");
    let plugin = plugin_registry_names().expect("插件注册表可解析");

    let core_set: BTreeSet<&str> = core.iter().map(String::as_str).collect();
    let plugin_set: BTreeSet<&str> = plugin.iter().map(String::as_str).collect();

    let overlap: Vec<&&str> = core_set.intersection(&plugin_set).collect();
    assert!(
        overlap.is_empty(),
        "同一命令不得同时在宿主与插件注册: {:?}",
        overlap
    );
}

#[test]
fn a7_plugin_command_list_matches_build_acl() {
    assert_eq!(
        command_to_permission("vault_init"),
        "allow-vault-init",
        "命令名到权限标识符的派生规则必须是 allow- + 蛇形转短横线"
    );
    assert_eq!(
        permission_to_command("allow-vault-init").as_deref(),
        Some("vault_init"),
        "权限标识符必须能逆向还原为命令名"
    );

    let features = plugin_feature_names();
    assert!(
        !features.is_empty(),
        "未发现任何 *_plugin.rs，解析器可能已失效"
    );

    let build_text = build_rs_text();
    assert!(
        build_text.contains(PLUGIN_DISCOVERY_MARKER),
        "build.rs 未按文件系统遍历插件真源（缺少 read_dir），派生逻辑可能已失效"
    );
    assert!(
        build_text.contains(PLUGIN_FILE_SUFFIX),
        "build.rs 未以 {} 后缀识别插件文件，派生逻辑可能已失效",
        PLUGIN_FILE_SUFFIX
    );

    let mut problems = Vec::new();
    let mut registered_total = 0usize;
    for feature in &features {
        let plugin_file = format!("{}{}", feature.replace('-', "_"), PLUGIN_FILE_SUFFIX);

        let registered: BTreeSet<String> = registered_names(&read(&plugin_file))
            .unwrap_or_else(|e| panic!("{} 注册表可解析: {}", plugin_file, e))
            .into_iter()
            .collect();
        registered_total += registered.len();
        let acl = acl_command_names(feature).unwrap_or_else(|e| panic!("{}", e));

        for (label, set) in [("注册表", &registered), ("ACL", &acl)] {
            if set.is_empty() {
                problems.push(format!("{}: {} 集合为空", feature, label));
            }
        }

        let only_left: Vec<&String> = registered.difference(&acl).collect();
        let only_right: Vec<&String> = acl.difference(&registered).collect();
        if !only_left.is_empty() {
            problems.push(format!("{}: 注册表有但 ACL 无 {:?}", feature, only_left));
        }
        if !only_right.is_empty() {
            problems.push(format!("{}: ACL 有但注册表无 {:?}", feature, only_right));
        }
    }

    assert!(
        registered_total >= ACL_PARSER_FLOOR,
        "插件注册表仅解析到 {} 条命令，低于下限 {}，解析器可能已失效",
        registered_total,
        ACL_PARSER_FLOOR
    );
    assert!(
        problems.is_empty(),
        "插件注册表与 ACL 不一致: {:?}",
        problems
    );
}

#[test]
fn a8_conf_applier_groups_match_config_structs() {
    for (module, group) in config_conf_file_groups() {
        assert_eq!(
            module, group,
            "{} 的 GROUP 常量 `{}` 必须与模块名逐字相等",
            module, group
        );
    }

    let core = config_conf_groups();
    assert!(
        !core.is_empty(),
        "assistant-config 未解析到任何 *_conf 模块，解析器可能已失效"
    );
    let feature = feature_conf_groups();
    assert!(
        !feature.is_empty(),
        "未从 crates/*/src/conf.rs 解析到任何配置组，解析器可能已失效"
    );

    let expected_set: BTreeSet<&str> = core
        .iter()
        .chain(feature.iter())
        .map(String::as_str)
        .collect();

    let actual = conf_applier_groups();
    let actual_set: BTreeSet<&str> = actual.iter().map(String::as_str).collect();

    let mut duplicates = Vec::new();
    for group in &actual_set {
        let count = actual.iter().filter(|item| *item == *group).count();
        if count > 1 {
            duplicates.push(format!("{} 注册 {} 次", group, count));
        }
    }

    let missing_applier: Vec<&&str> = expected_set.difference(&actual_set).collect();
    let unknown_group: Vec<&&str> = actual_set.difference(&expected_set).collect();

    assert!(
        duplicates.is_empty(),
        "conf applier 分组重复注册: {:?}",
        duplicates
    );
    assert!(
        missing_applier.is_empty(),
        "assistant-config 有结构体但无对应 applier: {:?}",
        missing_applier
    );
    assert!(
        unknown_group.is_empty(),
        "存在无对应 assistant-config 结构体的 applier 分组: {:?}",
        unknown_group
    );
}

#[test]
fn b1_mount_table_has_no_cfg() {
    let text = read("feature_mounts.rs");
    assert!(
        !text.contains(CFG_PREFIX),
        "feature_mounts.rs 不得出现任何 {}，表内容必须与开关无关",
        CFG_PREFIX
    );
}

#[test]
fn b2_feature_drift() {
    let manifest = manifest_feature_ids();
    assert!(
        !manifest.is_empty(),
        "config/features.json 未声明任何特性，漂移断言失去基准"
    );
    let cargo = cargo_feature_ids();
    let mounts = mounts_ids();
    let plugins = plugin_feature_names();

    let mut problems = Vec::new();
    for (label, ids) in [
        ("feature_mounts MOUNTS", &mounts),
        ("src/*_plugin.rs", &plugins),
    ] {
        if ids.join("|") != manifest.join("|") {
            problems.push(format!(
                "{} 的 {} ≠ 清单 {}",
                label,
                ids.join(", "),
                manifest.join(", ")
            ));
        }
    }

    let missing: Vec<String> = manifest_cargo_features()
        .into_iter()
        .filter(|feature| !cargo.iter().any(|cargo_feature| cargo_feature == feature))
        .collect();
    if !missing.is_empty() {
        problems.push(format!(
            "Cargo.toml [features] 缺少清单声明的 {}",
            missing.join(", ")
        ));
    }

    assert!(
        problems.is_empty(),
        "特性漂移（清单 / 挂载表 / 磁盘 三方全等，Cargo 侧为子集）: {:?}",
        problems
    );
}

#[test]
fn b3_callers_do_not_name_features() {
    let mut problems = Vec::new();
    for file in CALLER_FILES {
        let text = strip_rust_comments(&read(file));
        let literals = string_literals(&text);
        for feature in manifest_feature_ids() {
            if literals.contains(&feature) {
                problems.push(format!("{} 直接点名特性 {}", file, feature));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "调用方必须只遍历挂载表，不得出现特性名字面量: {:?}",
        problems
    );
}

#[test]
fn b4_feature_cfg_budget_and_mutual_blocks() {
    let features = manifest_feature_ids();
    let mut files = Vec::new();
    collect_rs_files(&src_root(), &mut files);

    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    for file in files {
        let text = fs::read_to_string(&file).expect("source file is readable");
        for feature in &features {
            let count = cfg_attr_count(&text, feature);
            if count > 0 {
                *totals.entry(feature.clone()).or_insert(0) += count;
            }
        }
    }

    let over_budget: Vec<String> = totals
        .iter()
        .filter(|(_, count)| **count > FEATURE_CFG_LIMIT)
        .map(|(name, count)| format!("{}: {} 行 cfg，上限 {}", name, count, FEATURE_CFG_LIMIT))
        .collect();
    assert!(
        over_budget.is_empty(),
        "单特性全后端 cfg 行数超限: {:?}",
        over_budget
    );

    let mut problems = Vec::new();
    for feature in &features {
        let plugin_file = format!("{}{}", feature.replace('-', "_"), PLUGIN_FILE_SUFFIX);
        let text = strip_rust_comments(&read(&plugin_file));
        if !text.contains(&enabled_block_marker(feature)) {
            problems.push(format!(
                "{} 缺少启用块 {}",
                plugin_file,
                enabled_block_marker(feature)
            ));
        }
        if !text.contains(&negated_block_marker(feature)) {
            problems.push(format!(
                "{} 缺少关闭块 {}",
                plugin_file,
                negated_block_marker(feature)
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "特性本体必须是「一对互斥块」形态: {:?}",
        problems
    );
}

#[test]
fn b5_disabled_block_mirrors_enabled_signatures() {
    let hooks = ["init", "on_setup", "on_exit"];
    let mut problems = Vec::new();

    for feature in manifest_feature_ids() {
        let plugin_file = format!("{}{}", feature.replace('-', "_"), PLUGIN_FILE_SUFFIX);
        let text = strip_rust_comments(&read(&plugin_file));
        let marker = negated_block_marker(&feature);
        let Some(split) = text.find(&marker) else {
            problems.push(format!("{} 缺少关闭块 {}", plugin_file, marker));
            continue;
        };
        let enabled = &text[..split];
        let disabled = &text[split..];

        for hook in hooks {
            let left = fn_signature(enabled, &plugin_file, hook);
            let right = fn_signature(disabled, &plugin_file, hook);
            if left != right {
                problems.push(format!(
                    "{} 的 {} 两侧签名不一致: 启用 {} / 关闭 {}",
                    plugin_file, hook, left, right
                ));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "关闭态实现必须与启用态同签名，否则挂载表函数指针类型失配: {:?}",
        problems
    );
}

#[test]
fn b6_cmd_mod_cfg_equals_feature_count() {
    let text = read("cmd/mod.rs");
    let features = manifest_feature_ids();
    let actual: Vec<String> = cfg_feature_names(&text)
        .into_iter()
        .filter(|name| features.iter().any(|feature| feature == name))
        .collect();

    assert_eq!(
        actual.len(),
        features.len(),
        "cmd/mod.rs 特性 cfg 应为每特性恒定 1 条，实际 {:?}",
        actual
    );

    for feature in &features {
        assert_eq!(
            cfg_attr_count(&text, feature),
            1,
            "cmd/mod.rs 的 {} 必须恰好 1 条 cfg 属性（空桩或 pub use 回流会超限）",
            feature
        );
    }
}

#[test]
fn b7_single_mount_table() {
    let text = strip_rust_comments(&read("feature_mounts.rs"));
    assert_eq!(
        text.matches(MOUNT_TABLE_TYPE).count(),
        1,
        "feature_mounts.rs 必须恰有 1 张 {} 表",
        MOUNT_TABLE_TYPE
    );
    assert!(
        text.contains(MOUNT_ACCESSOR),
        "feature_mounts.rs 缺少唯一读入口 {}",
        MOUNT_ACCESSOR
    );
    assert_eq!(
        text.matches("MOUNTS").count(),
        2,
        "MOUNTS 只允许「声明 1 次 + 被 mounts() 返回 1 次」，出现第二张表或旁路读取即失败"
    );
}

#[test]
fn b8_capabilities_split_per_feature() {
    let mut problems = Vec::new();
    let features = manifest_feature_ids();

    let default_text =
        fs::read_to_string(manifest_dir().join(CAPABILITIES_DIR).join("default.json"))
            .expect("capabilities/default.json 可读")
            .replace('\u{feff}', "");
    let default: serde_json::Value =
        serde_json::from_str(&default_text).expect("capabilities/default.json 是合法 JSON");
    let default_permissions: Vec<String> = default
        .get("permissions")
        .and_then(|value| value.as_array())
        .expect("default.json 的 permissions 是数组")
        .iter()
        .map(|value| value.as_str().unwrap_or_default().to_string())
        .collect();

    for feature in &features {
        let expected = format!("{}:default", feature);
        let path = manifest_dir()
            .join(CAPABILITIES_DIR)
            .join(format!("{}.json", feature));
        if !path.is_file() {
            problems.push(format!("缺少 {}", path.display()));
            continue;
        }
        let text = fs::read_to_string(&path).expect("capability 文件可读");
        let value: serde_json::Value =
            serde_json::from_str(&text).expect("capability 文件是合法 JSON");
        if value.get("identifier").and_then(|v| v.as_str()) != Some(feature.as_str()) {
            problems.push(format!("{} 的 identifier 不是 {}", path.display(), feature));
        }
        let permissions = value
            .get("permissions")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        if !permissions
            .iter()
            .any(|item| item.as_str() == Some(expected.as_str()))
        {
            problems.push(format!(
                "{} 的 permissions 缺少 {}",
                path.display(),
                expected
            ));
        }
        if default_permissions.contains(&expected) {
            problems.push(format!("default.json 仍授予 {}，特性权限未拆分", expected));
        }
    }

    assert!(
        problems.is_empty(),
        "capabilities 未按特性拆分: {:?}",
        problems
    );
}

#[test]
fn b9_single_handler_block_per_plugin() {
    let mut problems = Vec::new();
    for feature in manifest_feature_ids() {
        let plugin_file = format!("{}{}", feature.replace('-', "_"), PLUGIN_FILE_SUFFIX);
        let text = strip_rust_comments(&read(&plugin_file));
        let count = text.matches(REGISTRATION_OPEN).count();
        if count != 1 {
            problems.push(format!(
                "{} 有 {} 个 {} 注册块，必须恰为 1",
                plugin_file, count, REGISTRATION_OPEN
            ));
        }
    }

    let build_text = build_rs_text();
    assert!(
        build_text.contains("count() != 1"),
        "build.rs 未对 {} 块数量做同一判据的 panic",
        REGISTRATION_OPEN
    );

    assert!(
        problems.is_empty(),
        "插件注册块数量异常（build.rs 与 ACL 派生只读首个块）: {:?}",
        problems
    );
}
