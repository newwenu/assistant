// 受管目录：应用自有、由应用解析、只被应用自己读写的目录。
//
// 本模块是基根与无条件目录名的唯一拼串点，只在应用层使用。
// 工具调用的输入路径与工作区边界由 `assistant-tools::os::path` 承担，不在此处。
// 可选能力的目录名由该能力自身模块持有（`cmd/<feature>/`），本模块只提供 `data()` 基根。

pub mod dir_name;
pub mod form;
mod roots;

pub use form::strip_verbatim_prefix;
pub use roots::{AppDirs, DirError};
