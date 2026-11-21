use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InstallMascotAssetResultDto {
    /// 相对素材根目录的路径（如 `rive/custom.riv`）。前端经 `mascotAssetUrl` 拼接，不持有绝对路径。
    pub rel_path: String,
    pub bytes: u64,
    pub version: u64,
}

/// Rive 自定义素材的 ViewModel 属性名覆盖。缺省时前端按运行时 schema 自省推导。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RiveManifestPropertiesDto {
    #[serde(default)]
    #[ts(optional = nullable)]
    pub pose: Option<String>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub viseme: Option<String>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub primary_color: Option<String>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub secondary_color: Option<String>,
}

/// 自定义 Rive 素材的伴生声明（`rive/custom-manifest.json`）。
///
/// 只承载“意图”类信息：运行时自省能推导属性名与枚举值，但推不出
/// “哪个枚举值对应哪张脸”，那必须由素材作者或用户显式给出。
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RiveManifestDto {
    #[serde(default)]
    #[ts(optional = nullable)]
    pub state_machine: Option<String>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub properties: Option<RiveManifestPropertiesDto>,
    /// 该素材的 pose 枚举值全集，缺省时取自省结果。
    #[serde(default)]
    #[ts(optional = nullable)]
    pub poses: Option<Vec<String>>,
    /// 该素材的 viseme 枚举值全集，缺省时取自省结果。
    #[serde(default)]
    #[ts(optional = nullable)]
    pub visemes: Option<Vec<String>>,
    /// 运行时场景到 pose 的映射，优先级高于 `face_to_pose`。
    ///
    /// 场景是前端唯一的触发源（对话流开始/结束、工具调用、错误等），
    /// 表情只是场景的中间产物；按场景映射可以绕过“哪个表情在什么时候出现”
    /// 这层不可见的推导。缺省时回退到 `face_to_pose` 与名称启发式。
    #[serde(default)]
    #[ts(optional = nullable)]
    pub scene_to_pose: Option<std::collections::BTreeMap<String, String>>,
    /// 面部表情到 pose 的映射，缺省时按名称启发式匹配。
    #[serde(default)]
    #[ts(optional = nullable)]
    pub face_to_pose: Option<std::collections::BTreeMap<String, String>>,
    /// idle 时的轮转 pose，缺省时取 `poses` 去掉当前基准 pose。
    #[serde(default)]
    #[ts(optional = nullable)]
    pub idle_pose_cycle: Option<Vec<String>>,
}
