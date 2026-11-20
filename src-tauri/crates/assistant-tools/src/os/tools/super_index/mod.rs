//! B 私有公共层：行索引与行起点自证（I58 §7.2 / G18）。
//!
//! `super_read` 与 `super_edit` 都只**消费**本层，不各自复制判据：
//!
//! - `format`：索引文件格式、扫描建索引、原子落盘、载入校验（I58 §3.5）；
//! - `cache`：进程内整表缓存、单飞、容量淘汰、失效（I58 §4）；
//! - `proof`：行起点自证、单行解码、列推进与错误类型（I58 §4 / §3.2 F5·F6）。
//!
//! 判据单源是硬约束：两端自证是「区间正确」的唯一判据，复制两份后任一侧改判据
//! 都会让读写对同一文件给出不同结论，而这种漂移不会被任何单测发现（§12 R14）。
//! 结构性判据见 G18：删掉 `proof` 后两个工具都无法编译。

pub(crate) mod cache;
pub(crate) mod format;
pub(crate) mod proof;

use std::path::Path;

use crate::runtime::storage::{dir_tree_stats, purge_tree_files, DirStats, PurgeReport};

/// 落盘索引文件的扩展名（`format::index_file_name` 的唯一后缀）。
const INDEX_SUFFIX: &str = ".idx";

/// `E4` 的回收结果（I61 §3.3 `A5`）：成功条数 / 成功字节 / 失败条数 / 判据排除条数。
pub type IndexPurgeReport = PurgeReport;

/// 只读扫描索引根；遍历规则与 `cache` 的淘汰口径一致（递归、跳符号链接、
/// 跳非常规文件），保证「统计多少」与「能删多少」不会各说一套。
pub fn index_stats(index_root: &Path) -> std::io::Result<DirStats> {
    dir_tree_stats(index_root)
}

/// `E4` 的回收入口（I61 `A5` 全量重建缓存）：删掉索引根下所有 `*.idx`。
///
/// 与 [`index_stats`] 共用同一遍历实现，只删可重建的缓存文件，不提供「按 key 删」
/// （文件名不外泄给管理器）；内存 LRU 不参与统计、也不因删除而失效（后续 miss 重建）。
///
/// 伴生残留（`*.tmp` / `-journal` / `-wal` / `-shm` / `.lock`）既不计入
/// [`index_stats`]，也不在此删除——写入路径自己在 `atomic_write` 后清扫。
/// 索引根里的非 `.idx` 外来文件同理：统计会看到它，回收不动它，因此
/// 「预计字节」可能略大于「实际删除字节」，偏差来源可解释（I61 V7）。
pub fn purge_index_files(index_root: &Path) -> IndexPurgeReport {
    purge_tree_files(index_root, INDEX_SUFFIX)
}
