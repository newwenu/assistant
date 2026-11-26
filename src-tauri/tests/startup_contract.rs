// 参考: docs/store/I75-startup-layering-spec.md §8 G1 / G2 / G3 — 启动链路门禁
//
// 断言一律读源码：`build` 是函数指针，运行期不可内省；测试也不链接 tauri 运行期，
// 否则门禁会退化成「能起进程」才成立。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

mod support;

use support::scan::{
    collect_rs_files, manifest_dir, read, src_root, strip_rust_comments, tauri_conf_windows,
};

const RESOURCES_FILE: &str = "boot/resources.rs";
const TABLE_OPEN: &str = "pub static RESOURCES: &[Res] = &[";
const PRIMARY_ENTRY_SET: &str = "PRIMARY_ENTRY";
const BOOT_REQUIRED_SET: &str = "BOOT_REQUIRED";
const OBSERVABLE_SET: &str = "OBSERVABLE";
const BEST_EFFORT_SET: &str = "BEST_EFFORT";
const ACTIONABLE_SET: &str = "ACTIONABLE";
const PLUGINS_FILE: &str = "plugins.rs";
const ATTACH_OPEN: &str = "pub fn attach(";
const MOUNT_LOOP: &str = "for mount in crate::feature_mounts::mounts()";
const SINGLE_INSTANCE: &str = "tauri_plugin_single_instance::init";
const LOG_PLUGIN: &str = ".plugin(log_plugin(";
const BOOT_DIR: &str = "boot";
const BOOT_CONF_FILE: &str = "boot/conf.rs";
const APP_DIRS_RESOLVE: &str = "AppDirs::resolve(";
const CONFIG_READ_POINTS: [&str; 2] = ["AppConf::load(", "OrbConf::load("];
/// 启动路径白名单：这些文件/目录内不得出现第二处 `AppDirs::resolve()`（G3）。
const STARTUP_PATH_FILES: [&str; 5] = [
    "main.rs",
    "plugins.rs",
    "tool_setup.rs",
    "vault_plugin.rs",
    "timekeeper_plugin.rs",
];
const STARTUP_PATH_DIRS: [&str; 2] = [BOOT_DIR, "window"];
const APP_DIRS_OWNER: &str = "main.rs";

/// 只取 `attach` 函数体内、特性挂载循环之前的文本。
///
/// 限定范围是必要的：`fn log_plugin()` 的定义在文件后半段，全文查找会在函数被
/// 挪位置或新增辅助函数后误判；特性插件循环排在 `log_plugin()` 之后，不参与比较。
fn attach_chain(text: &str) -> &str {
    let start = text
        .find(ATTACH_OPEN)
        .unwrap_or_else(|| panic!("{PLUGINS_FILE} 缺少 {ATTACH_OPEN}"));
    let tail = &text[start..];
    let end = tail.find(MOUNT_LOOP).unwrap_or(tail.len());
    &tail[..end]
}

#[test]
fn g2_single_instance_registers_before_log_plugin() {
    let text = read(PLUGINS_FILE);
    let chain = attach_chain(&text);

    let single_instance = chain.find(SINGLE_INSTANCE).unwrap_or_else(|| {
        panic!("{PLUGINS_FILE} 的 attach 链缺少 {SINGLE_INSTANCE}：单实例守卫已被移除或改名")
    });
    let log_plugin = chain.find(LOG_PLUGIN).unwrap_or_else(|| {
        panic!("{PLUGINS_FILE} 的 attach 链缺少 {LOG_PLUGIN}：日志插件注册已被移除或改名")
    });

    assert!(
        single_instance < log_plugin,
        "single_instance 必须注册在 log_plugin 之前：插件 setup 按注册顺序执行，\
         第二个实例若晚于日志插件退出，会先把首实例正在写入的日志文件轮转掉，\
         崩溃前最后几行日志随之丢失"
    );
}

#[test]
fn g2_attach_chain_is_locatable() {
    let text = read(PLUGINS_FILE);
    let chain = attach_chain(&text);
    assert!(
        !chain.is_empty(),
        "{PLUGINS_FILE} 的 attach 链为空：插件注册可能已迁出 attach，门禁需要同步更新"
    );
}

fn relative(file: &Path) -> String {
    file.strip_prefix(src_root())
        .expect("source file lives under src")
        .to_string_lossy()
        .replace('\\', "/")
}

/// 源码里的表项行：只保留 G1 需要的字段。
struct Row {
    id: String,
    lane: String,
    needs: Vec<String>,
    /// `build` 目标（文件, 函数）。`build` 是函数指针，运行期不可内省，
    /// 只有源码能给出「哪个 id 由哪个函数装配」。
    build: (String, String),
}

fn between<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)? + open.len();
    let tail = &text[start..];
    let end = tail.find(close)?;
    Some(&tail[..end])
}

fn quoted(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('"') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('"') else {
            break;
        };
        out.push(after[..end].to_string());
        rest = &after[end + 1..];
    }
    out
}

/// 取 `pub static RESOURCES: &[Res] = &[ ... ];` 的表体。
fn table_body(table: &str) -> &str {
    let start = table
        .find(TABLE_OPEN)
        .unwrap_or_else(|| panic!("{RESOURCES_FILE} 缺少 {TABLE_OPEN}"));
    let tail = &table[start + TABLE_OPEN.len()..];
    let end = tail
        .find("\n];")
        .unwrap_or_else(|| panic!("{RESOURCES_FILE} 的 RESOURCES 表未闭合"));
    &tail[..end]
}

fn rows(table: &str) -> Vec<Row> {
    let body = table_body(table);
    let mut starts = Vec::new();
    let mut cursor = 0;
    while let Some(pos) = body[cursor..].find("Res {") {
        let at = cursor + pos;
        starts.push(at);
        cursor = at + "Res {".len();
    }

    let mut out = Vec::new();
    for (index, start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(body.len());
        let block = &body[*start..end];
        let id = between(block, "id: \"", "\"")
            .unwrap_or_else(|| panic!("{RESOURCES_FILE} 有表项缺 id"))
            .to_string();
        let lane = between(block, "lane: Lane::", ",")
            .unwrap_or_else(|| panic!("表项 {id} 缺 lane"))
            .trim()
            .to_string();
        let needs = quoted(
            between(block, "needs: &[", "]").unwrap_or_else(|| panic!("表项 {id} 缺 needs")),
        );
        let target = between(block, "build: crate::boot::build::", ",")
            .unwrap_or_else(|| panic!("表项 {id} 缺 build"))
            .trim()
            .to_string();
        let Some((file, func)) = target.split_once("::") else {
            panic!("表项 {id} 的 build 目标 {target} 不是 文件::函数 形式");
        };
        out.push(Row {
            id,
            lane,
            needs,
            build: (file.to_string(), func.to_string()),
        });
    }
    out
}

fn set_members(table: &str, name: &str) -> Vec<String> {
    let open = format!("pub const {name}: &[&str] = &[");
    let body =
        between(table, &open, "];").unwrap_or_else(|| panic!("{RESOURCES_FILE} 缺少集合 {name}"));
    quoted(body)
}

/// 取 `fn <func>` 函数体；函数之间的边界用 `pub fn` 定位。
fn function_body<'a>(source: &'a str, func: &str) -> &'a str {
    let marker = format!("fn {func}(");
    let Some(start) = source.find(&marker) else {
        return "";
    };
    let tail = &source[start + marker.len()..];
    let end = tail.find("\npub fn ").unwrap_or(tail.len());
    &tail[..end]
}

fn resource_sets(table: &str) -> [(&'static str, Vec<String>); 4] {
    [
        (PRIMARY_ENTRY_SET, set_members(table, PRIMARY_ENTRY_SET)),
        (BOOT_REQUIRED_SET, set_members(table, BOOT_REQUIRED_SET)),
        (OBSERVABLE_SET, set_members(table, OBSERVABLE_SET)),
        (BEST_EFFORT_SET, set_members(table, BEST_EFFORT_SET)),
    ]
}

#[test]
fn g1_resource_table_is_self_consistent() {
    let table = strip_rust_comments(&read(RESOURCES_FILE));
    let rows = rows(&table);
    assert!(
        !rows.is_empty(),
        "{RESOURCES_FILE} 未解析出任何表项：表结构可能已改名，门禁需同步更新"
    );

    let ids: Vec<&str> = rows.iter().map(|row| row.id.as_str()).collect();
    let mut problems = Vec::new();

    let mut unique = BTreeSet::new();
    for id in &ids {
        if !unique.insert(*id) {
            problems.push(format!("id 重复: {id}"));
        }
    }

    for (index, row) in rows.iter().enumerate() {
        for need in &row.needs {
            match ids.iter().position(|id| id == need) {
                None => problems.push(format!("{} 的 needs 指向表外 id: {need}", row.id)),
                Some(pos) if pos >= index => {
                    problems.push(format!("{} 的 needs 未声明在前: {need}", row.id))
                }
                Some(_) => {}
            }
        }
    }

    let sets = resource_sets(&table);
    let mut owners: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for (name, members) in &sets {
        for id in members {
            owners.entry(id.as_str()).or_default().push(*name);
        }
    }
    for (id, names) in &owners {
        if names.len() > 1 {
            problems.push(format!("{id} 同时属于多份集合: {}", names.join(" / ")));
        }
        if !ids.contains(id) {
            problems.push(format!("集合成员不在表内: {id}"));
        }
    }
    for id in &ids {
        if !owners.contains_key(id) {
            problems.push(format!("{id} 不在四份集合中，severity 无法派生"));
        }
    }

    let actionable = set_members(&table, ACTIONABLE_SET);
    let observable = set_members(&table, OBSERVABLE_SET);
    for id in &actionable {
        if !observable.contains(id) {
            problems.push(format!("ACTIONABLE 成员 {id} 不在 OBSERVABLE"));
        }
    }

    for id in set_members(&table, PRIMARY_ENTRY_SET)
        .iter()
        .chain(set_members(&table, BOOT_REQUIRED_SET).iter())
    {
        match rows.iter().find(|row| row.id == *id) {
            Some(row) if row.lane == "Boot" => {}
            Some(row) => problems.push(format!("{id} 属致命集合但泳道是 {}", row.lane)),
            None => problems.push(format!("{id} 属致命集合但不在表内")),
        }
    }

    for row in &rows {
        let fatal = set_members(&table, PRIMARY_ENTRY_SET).contains(&row.id)
            || set_members(&table, BOOT_REQUIRED_SET).contains(&row.id);
        if row.lane == "Idle" && fatal {
            problems.push(format!("{} 属 Idle 泳道却是 Fatal", row.id));
        }
    }

    let severity_body = function_body(&table, "severity");
    for name in [PRIMARY_ENTRY_SET, BOOT_REQUIRED_SET, OBSERVABLE_SET] {
        if !severity_body.contains(name) {
            problems.push(format!("severity 未由集合 {name} 派生，档位可能被手写"));
        }
    }

    assert!(
        problems.is_empty(),
        "资源表自洽断言失败：\n{}",
        problems.join("\n")
    );
}

#[test]
fn g1_needs_closure_matches_source_usage() {
    let table = strip_rust_comments(&read(RESOURCES_FILE));
    let rows = rows(&table);
    let mut problems = Vec::new();

    for row in &rows {
        let (file, func) = &row.build;
        let source = read(&format!("boot/build/{file}.rs"));
        let body = function_body(&source, func);
        if body.is_empty() {
            problems.push(format!("{file}.rs 中找不到 {func}，文件↔id 映射失效"));
            continue;
        }
        if body.contains("ctx.boot()") && !row.needs.iter().any(|need| need == "boot_conf") {
            problems.push(format!(
                "{func} 读取 ctx.boot()，但 {} 的 needs 缺 boot_conf",
                row.id
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "needs 闭包与源码不一致：\n{}",
        problems.join("\n")
    );
}

#[test]
fn g3_boot_reads_config_once() {
    let mut files = Vec::new();
    collect_rs_files(&src_root().join(BOOT_DIR), &mut files);
    let mut problems = Vec::new();

    for file in files {
        let name = relative(&file);
        if name == BOOT_CONF_FILE {
            continue;
        }
        let text = strip_rust_comments(&fs::read_to_string(&file).expect("source file readable"));
        for point in CONFIG_READ_POINTS {
            if text.contains(point) {
                problems.push(format!("{name} 出现二次配置读盘点 {point}"));
            }
        }
    }

    assert!(
        problems.is_empty(),
        "boot/ 下除 {BOOT_CONF_FILE} 外不得读配置：\n{}",
        problems.join("\n")
    );
}

#[test]
fn g3_app_dirs_resolved_once() {
    let mut files: Vec<PathBuf> = STARTUP_PATH_FILES
        .iter()
        .map(|name| src_root().join(name))
        .collect();
    for dir in STARTUP_PATH_DIRS {
        collect_rs_files(&src_root().join(dir), &mut files);
    }

    let mut problems = Vec::new();
    for file in files {
        if !file.is_file() {
            problems.push(format!("启动路径白名单文件缺失: {}", file.display()));
            continue;
        }
        let name = relative(&file);
        let text = strip_rust_comments(&fs::read_to_string(&file).expect("source file readable"));
        if name != APP_DIRS_OWNER && text.contains(APP_DIRS_RESOLVE) {
            problems.push(format!(
                "{name} 重复解析 AppDirs，唯一解析点应在 {APP_DIRS_OWNER}"
            ));
        }
    }

    assert!(
        problems.is_empty(),
        "启动路径目录解析点必须唯一：\n{}",
        problems.join("\n")
    );
}

/// G4：窗口单一来源。`tauri.conf.json` 不得再声明窗口，规格只在 `WINDOW_SPECS` 一份。
#[test]
fn g4_tauri_conf_has_no_windows() {
    let windows = tauri_conf_windows();
    assert!(
        windows.is_empty(),
        "tauri.conf.json 的 app.windows 必须为空数组，实际有 {} 项：{}",
        windows.len(),
        serde_json::Value::Array(windows)
    );
}

/// G5：能力门禁。`cmd/`、`adapters/` 只能经 `boot::access::*` 取用已登记能力，
/// 不得直取 Tauri 状态或触碰 registry 内部符号（绕过 `ensure` 的第二种入口）。
///
/// 类型清单由 `boot/access.rs` 访问器返回类型源码扫描派生，不手工维护：新增访问器即自动纳入门禁。
const ACCESS_FILE: &str = "boot/access.rs";
const ACCESS_ARROW: &str = "-> Result<Arc<";
const REGISTRY_SYMBOL: &str = "boot::registry::";
const GATE_DIRS: [&str; 2] = ["cmd", "adapters"];
const STATE_ACCESS: &str = "state::<";

fn registered_access_types() -> BTreeSet<String> {
    let text = strip_rust_comments(&read(ACCESS_FILE));
    let mut out = BTreeSet::new();
    let mut rest = text.as_str();
    while let Some(pos) = rest.find("pub fn ") {
        let tail = &rest[pos..];
        let end = tail[4..]
            .find("\npub fn ")
            .map(|offset| offset + 4)
            .unwrap_or(tail.len());
        let body = &tail[..end];
        if let Some(arrow) = body.find(ACCESS_ARROW) {
            let after = &body[arrow + ACCESS_ARROW.len()..];
            if let Some(close) = after.find('>') {
                let inner = after[..close].trim();
                let name = inner
                    .trim_start_matches("dyn ")
                    .rsplit("::")
                    .next()
                    .unwrap_or(inner)
                    .trim();
                out.insert(name.to_string());
            }
        }
        rest = &rest[pos + "pub fn ".len()..];
    }
    out
}

#[test]
fn g5_accessor_type_list_is_locatable() {
    let registered = registered_access_types();
    assert!(
        !registered.is_empty(),
        "{ACCESS_FILE} 未解析出任何访问器返回类型：签名形态可能已变化，门禁需同步更新"
    );
}

#[test]
fn g5_command_layer_uses_accessors() {
    let registered = registered_access_types();
    let mut files = Vec::new();
    for dir in GATE_DIRS {
        collect_rs_files(&src_root().join(dir), &mut files);
    }

    let mut problems = Vec::new();
    for file in files {
        let name = relative(&file);
        let text = strip_rust_comments(&fs::read_to_string(&file).expect("source file readable"));
        if text.contains(REGISTRY_SYMBOL) {
            problems.push(format!("{name} 出现 {REGISTRY_SYMBOL} 内部符号"));
        }

        let mut cursor = 0;
        while let Some(pos) = text[cursor..].find(STATE_ACCESS) {
            let at = cursor + pos;
            let after = &text[at + STATE_ACCESS.len()..];
            let Some(close) = after.find('>') else {
                cursor = at + STATE_ACCESS.len();
                continue;
            };
            let inner = after[..close].trim();
            let type_name = inner
                .trim_start_matches("Arc<")
                .trim_start_matches("dyn ")
                .rsplit("::")
                .next()
                .unwrap_or(inner)
                .trim();
            if registered.contains(type_name) {
                let is_try = at >= 4 && &text[at - 4..at] == "try_";
                problems.push(format!(
                    "{name} 直取已登记能力 {type_name}（{}），应经 crate::boot::* 访问器",
                    if is_try { "try_state::<>" } else { "state::<>" }
                ));
            }
            cursor = at + STATE_ACCESS.len();
        }
    }

    assert!(
        problems.is_empty(),
        "能力门禁被绕过：\n{}",
        problems.join("\n")
    );
}

/// G4 后半：生成物新鲜度由 `contract:verify` 的 `git diff --exit-code` 保证。
#[test]
fn g4_window_requirements_generated_is_verified() {
    let package_path = manifest_dir().join("..").join("package.json");
    let text = fs::read_to_string(&package_path)
        .unwrap_or_else(|e| panic!("{} 可读: {}", package_path.display(), e));
    let value: serde_json::Value = serde_json::from_str(&text).expect("package.json 是合法 JSON");
    let verify = value
        .get("scripts")
        .and_then(|scripts| scripts.get("contract:verify"))
        .and_then(|verify| verify.as_str())
        .unwrap_or_else(|| panic!("package.json 缺少 scripts.contract:verify"));

    for needle in [
        "windowRequirements.generated.ts",
        "git ls-files --error-unmatch",
        "git diff --exit-code",
    ] {
        assert!(
            verify.contains(needle),
            "contract:verify 缺少 {needle}：{verify}"
        );
    }
}
