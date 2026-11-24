use assistant_tools::ToolTier;

mod audit;
mod grants;
mod project_rules;

pub use audit::*;
pub use grants::*;
pub use project_rules::*;

/// 沙箱档位展示标签：列表作用域摘要与审计摘要共用同一词表。
fn tier_label(tier: ToolTier) -> &'static str {
    match tier {
        ToolTier::Sandbox => "sandbox",
        ToolTier::Os => "os",
        ToolTier::External => "external",
    }
}

/// 取哈希前 8 位：摘要里只给指纹前缀，完整值对用户无意义。
fn short_hash(value: &str) -> &str {
    value.get(..8).unwrap_or(value)
}
