use encoding_rs::{Encoding, UTF_8};
use std::path::Path;

use crate::os::text_encoding::{detect_and_decode, encode_text, writes_bom};

/// 目标编码与 BOM 状态（G1 / G5 的解析结果）。
#[derive(Debug, Clone, Copy)]
pub(super) struct TargetEncoding {
    encoding: &'static Encoding,
    has_bom: bool,
}

impl TargetEncoding {
    pub(super) fn name(&self) -> &'static str {
        self.encoding.name()
    }

    /// UTF-8 无 BOM：走原有直写路径，不经过 `encode_text`，G2 / G3 / G4 无额外开销。
    pub(super) fn is_plain_utf8(&self) -> bool {
        self.encoding == UTF_8 && !self.has_bom
    }

    /// 实际会写进文件的 BOM 状态，用于刷新回执（UTF-16 恒写，其余编码无 BOM）。
    pub(super) fn written_bom(&self) -> bool {
        writes_bom(self.encoding, self.has_bom)
    }
}

/// G1：编码解析顺序为 `encoding` 参数 → 已存在文件的回执 → UTF-8。
///
/// 回执继承让模型不必知道文件原本是什么编码：`read` 已经发现过，写侧直接复用。
pub(super) fn resolve_target_encoding(
    explicit: Option<&'static Encoding>,
    inherited: Option<&'static Encoding>,
    inherited_bom: bool,
) -> TargetEncoding {
    TargetEncoding {
        encoding: explicit.or(inherited).unwrap_or(UTF_8),
        has_bom: inherited_bom,
    }
}

/// 生成落盘字节（G2 / G5）。
///
/// 追加一律走「解码现有内容 → 拼接 → 整体编码」，**不**把两段字节直接拼接——编码边界处的
/// 字节拼接会产生非法序列。唯一的直写例外是「现有内容本身是合法 UTF-8 且无 BOM、目标也是
/// UTF-8 无 BOM」：此时解码 / 编码都是恒等变换，省掉整文件重编码的开销。
///
/// 解码现有内容用**现有文件的编码**（回执），不是目标编码：显式转码时两者可以不同。
pub(super) fn build_payload(
    path: &Path,
    existing: Option<&[u8]>,
    content: &str,
    append: bool,
    target: TargetEncoding,
    source: Option<&'static Encoding>,
) -> Result<Vec<u8>, String> {
    let text = match (existing, append) {
        (Some(bytes), true) => {
            let source_encoding = source.unwrap_or(UTF_8);
            let plain_source = source_encoding == UTF_8
                && Encoding::for_bom(bytes).is_none()
                && std::str::from_utf8(bytes).is_ok();
            if target.is_plain_utf8() && plain_source {
                let mut combined = Vec::with_capacity(bytes.len() + content.len());
                combined.extend_from_slice(bytes);
                combined.extend_from_slice(content.as_bytes());
                return Ok(combined);
            }
            let decoded = detect_and_decode(bytes, Some(source_encoding))
                .map_err(|error| format!("'{}': {}", path.display(), error))?;
            format!("{}{}", decoded.text, content)
        }
        _ => content.to_string(),
    };

    if target.is_plain_utf8() {
        return Ok(text.into_bytes());
    }

    encode_text(&text, target.encoding, target.has_bom)
        .map_err(|error| format!("'{}': {}", path.display(), error))
}
