use std::sync::MutexGuard;

use assistant_crypto::vault_crypto::derive_meta_key;
use assistant_crypto::vault_crypto::types::{DbKey, Dek, Kek, MetaKey};
use rusqlite::Connection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockReason {
    User,
    AutoLock,
    ScreenLock,
    AppExit,
    Reset,
    Panic,
}

impl LockReason {
    /// 审计 detail 用的稳定字符串（封闭集合，禁止自由文本）。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::AutoLock => "auto_lock",
            Self::ScreenLock => "screen_lock",
            Self::AppExit => "app_exit",
            Self::Reset => "reset",
            Self::Panic => "panic",
        }
    }
}

pub struct VaultSession {
    pub(crate) conn: Connection,
    pub(crate) dek: Dek,
    pub(crate) db_key: DbKey,
    pub(crate) meta_key: MetaKey,
    pub(crate) dek_version: u32,
    pub(crate) epoch: u64,
    pub(crate) kek: Option<Kek>,
}

impl VaultSession {
    pub fn new(conn: Connection, dek: Dek, db_key: DbKey, dek_version: u32) -> Self {
        let meta_key = derive_meta_key(&dek);
        Self {
            conn,
            dek,
            db_key,
            meta_key,
            dek_version,
            epoch: 0,
            kek: None,
        }
    }

    /// 口令解锁/初始化时缓存 `kek`：`rotate_dek` 重包 `dek_new` 需要当前 `KEK`，
    /// 生物识别与恢复码会话没有 `KEK`，此时 `kek` 为 `None`。
    pub fn with_kek(mut self, kek: Kek) -> Self {
        self.kek = Some(kek);
        self
    }

    pub fn kek(&self) -> Option<&Kek> {
        self.kek.as_ref()
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn dek(&self) -> &Dek {
        &self.dek
    }

    pub fn db_key(&self) -> &DbKey {
        &self.db_key
    }

    pub fn meta_key(&self) -> &MetaKey {
        &self.meta_key
    }

    pub fn dek_version(&self) -> u32 {
        self.dek_version
    }

    pub fn epoch(&self) -> u64 {
        self.epoch
    }
}

pub struct SessionConnGuard<'a> {
    pub(super) guard: MutexGuard<'a, Option<VaultSession>>,
}

impl<'a> SessionConnGuard<'a> {
    pub fn as_ref(&self) -> Option<&Connection> {
        self.guard.as_ref().map(|session| &session.conn)
    }
}

pub struct SessionDekGuard<'a> {
    pub(super) guard: MutexGuard<'a, Option<VaultSession>>,
}

impl<'a> SessionDekGuard<'a> {
    pub fn as_ref(&self) -> Option<&Dek> {
        self.guard.as_ref().map(|session| &session.dek)
    }

    pub fn dek_version(&self) -> Option<u32> {
        self.guard.as_ref().map(|session| session.dek_version)
    }
}
