pub mod agent_delete;
pub mod agent_edit;
pub mod agent_glob;
pub mod agent_grep;
pub mod agent_list;
pub mod agent_mkdir;
pub mod agent_move;
pub mod agent_read;
pub mod agent_shell;
pub mod agent_write;
pub mod fs_link;
mod meta;
pub mod super_edit;
pub(crate) mod super_index;
pub mod super_read;

pub use meta::{agent_ask_user, agent_request_tool};
pub use super_index::{index_stats, purge_index_files, IndexPurgeReport};
