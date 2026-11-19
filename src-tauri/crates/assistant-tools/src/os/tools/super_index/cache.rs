//! 索引缓存与单飞（I58 §4）。
//!
//! 缓存只存**整张起点表**，不存任何文件内容：内容永远是本次从磁盘现读的，
//! 所以「内容变了」本身从不需要失效。正确性由 `proof` 的行起点自证承担，
//! 缓存只承担「别重复扫描」这件事（§4 粗筛 / 单飞 / 容量）。

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::time::Instant;

use encoding_rs::Encoding;

use crate::os::tools::super_index::format;

/// 内存索引缓存上限：按「起点表字节数」计，`(lineCount + 1) * 8`（I58 §4）。
const INDEX_CACHE_BYTES: usize = 64 * 1024 * 1024;

/// 缓存键：同一路径的索引随 `revision` 变化；索引根不同则不是同一份缓存。
type CacheKey = (PathBuf, PathBuf, String);

fn cache_key(index_root: &Path, source: &Path, revision: &str) -> CacheKey {
    (
        index_root.to_path_buf(),
        source.to_path_buf(),
        revision.to_string(),
    )
}

/// 一份内存索引：整张起点表 + 淘汰时要删的落盘文件。
struct CacheEntry {
    offsets: Arc<Vec<u64>>,
    bytes: usize,
    file: PathBuf,
    /// 单调递增的使用序号，LRU 淘汰取最小值。
    tick: u64,
}

#[derive(Default)]
struct CacheState {
    entries: HashMap<CacheKey, CacheEntry>,
    bytes: usize,
}

/// 单飞登记：同一 `(path, revision)` 并发只建一次，其余等待后取同一结果。
struct Flight {
    state: Mutex<FlightState>,
    done: Condvar,
}

#[derive(Default)]
struct FlightState {
    finished: bool,
    error: Option<String>,
}

struct IndexCache {
    state: Mutex<CacheState>,
    flights: Mutex<HashMap<CacheKey, Arc<Flight>>>,
    tick: AtomicU64,
}

static INDEX_CACHE: OnceLock<IndexCache> = OnceLock::new();

fn index_cache() -> &'static IndexCache {
    INDEX_CACHE.get_or_init(|| IndexCache {
        state: Mutex::new(CacheState::default()),
        flights: Mutex::new(HashMap::new()),
        tick: AtomicU64::new(0),
    })
}

/// 锁中毒时继续用内部值：缓存与单飞登记都是派生状态，没有需要保护的业务不变量。
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|error| error.into_inner())
}

impl IndexCache {
    /// 命中即返回整张起点表，同时刷新 LRU 序号。
    fn lookup(&self, key: &CacheKey) -> Option<Arc<Vec<u64>>> {
        let tick = self.tick.fetch_add(1, Ordering::Relaxed);
        let mut state = lock(&self.state);
        let entry = state.entries.get_mut(key)?;
        entry.tick = tick;
        Some(Arc::clone(&entry.offsets))
    }

    /// 写入并做字节级 LRU 淘汰；淘汰连带删除落盘索引（派生数据不留残余）。
    fn insert(&self, key: CacheKey, offsets: Arc<Vec<u64>>, file: PathBuf) {
        let tick = self.tick.fetch_add(1, Ordering::Relaxed);
        let bytes = offsets.len() * std::mem::size_of::<u64>();
        let mut state = lock(&self.state);
        if let Some(previous) = state.entries.remove(&key) {
            state.bytes -= previous.bytes;
        }
        state.bytes += bytes;
        state.entries.insert(
            key,
            CacheEntry {
                offsets,
                bytes,
                file,
                tick,
            },
        );

        while state.bytes > INDEX_CACHE_BYTES {
            let Some(victim) = state
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.tick)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            if let Some(entry) = state.entries.remove(&victim) {
                state.bytes -= entry.bytes;
                let _ = std::fs::remove_file(&entry.file);
            }
        }
    }

    /// 进入单飞：返回 `(登记项, 是否 leader)`。
    fn begin_flight(&self, key: &CacheKey) -> (Arc<Flight>, bool) {
        let mut flights = lock(&self.flights);
        if let Some(flight) = flights.get(key) {
            return (Arc::clone(flight), false);
        }
        let flight = Arc::new(Flight {
            state: Mutex::new(FlightState::default()),
            done: Condvar::new(),
        });
        flights.insert(key.clone(), Arc::clone(&flight));
        (flight, true)
    }

    /// 等待 leader 收尾；返回它的错误（`None` 表示成功，调用方回到循环取缓存）。
    fn wait_flight(&self, flight: &Flight) -> Option<String> {
        let mut state = lock(&flight.state);
        while !state.finished {
            state = flight
                .done
                .wait(state)
                .unwrap_or_else(|error| error.into_inner());
        }
        state.error.clone()
    }

    /// leader 收尾：先写结果再唤醒；登记项此时才移出，后来者只会走「查缓存 → 建索引」。
    fn finish_flight(&self, key: &CacheKey, flight: &Flight, error: Option<String>) {
        {
            let mut state = lock(&flight.state);
            state.finished = true;
            state.error = error;
        }
        lock(&self.flights).remove(key);
        flight.done.notify_all();
    }
}

/// 索引状态：本次调用是新建还是复用落盘索引。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndexState {
    Built,
    Reused,
}

impl IndexState {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Built => "built",
            Self::Reused => "reused",
        }
    }
}

/// 路径失效：纯缓存卫生，**不承担正确性**（I58 §4）。
///
/// 正确性由 `proof` 的行起点自证兜底，对所有写入方一视同仁，因此 A 侧通用写工具
/// （`edit` / `write` / `delete` / `move`）完全不调用它、也不感知本模块存在。
/// 唯一的调用方是 B 自己的 `super_edit`：刚写完的文件其旧索引必然失效，
/// 主动丢弃可省掉一次「自证失败 → 重建」的往返。
///
/// 修订号已变时缓存键自然不命中，但旧条目仍占内存与磁盘，所以显式丢弃：
/// 移除 `path` 自身及其子树下的所有索引缓存，并删掉对应落盘文件。
/// 走组件级前缀判等（`os::path::is_component_prefix`），`a/bc` 不属于 `a/b`。
pub(crate) fn invalidate_path(path: &Path) {
    let cache = index_cache();
    let mut state = lock(&cache.state);
    let victims: Vec<CacheKey> = state
        .entries
        .iter()
        .filter(|(key, _)| crate::os::path::is_component_prefix(&key.1, path))
        .map(|(key, _)| key.clone())
        .collect();

    for key in victims {
        if let Some(entry) = state.entries.remove(&key) {
            state.bytes -= entry.bytes;
            let _ = std::fs::remove_file(&entry.file);
        }
    }
}

/// 取索引：内存缓存命中 → 单飞建索引。返回 `(整张起点表, 状态, 建索引耗时)`。
///
/// 单飞的意义不只是省一次扫描：同一 `(path, revision)` 并发进来时，落盘写入与
/// `file_revision` 校验都只做一次，避免互相看到写了一半的索引文件（I58 §4）。
pub(crate) fn index_for(
    index_root: &Path,
    source_path: &Path,
    revision: &str,
    file_bytes: u64,
    explicit: Option<&'static Encoding>,
) -> Result<(Arc<Vec<u64>>, IndexState, u64), String> {
    let key = cache_key(index_root, source_path, revision);
    let cache = index_cache();

    loop {
        if let Some(offsets) = cache.lookup(&key) {
            return Ok((offsets, IndexState::Reused, 0));
        }

        let (flight, leader) = cache.begin_flight(&key);
        if !leader {
            // 等待期间 leader 可能成功也可能失败：成功则下一轮命中缓存，
            // 失败则把同一个业务错误直接转给所有等待者，不重复扫描一遍。
            if let Some(error) = cache.wait_flight(&flight) {
                return Err(error);
            }
            continue;
        }

        // 成为 leader 后必须再确认一次缓存：上一个 leader 可能刚写完缓存就退出登记。
        if let Some(offsets) = cache.lookup(&key) {
            cache.finish_flight(&key, &flight, None);
            return Ok((offsets, IndexState::Reused, 0));
        }

        let started = Instant::now();
        let result = build_offsets(index_root, source_path, revision, file_bytes, explicit);
        if let Ok(offsets) = &result {
            cache.insert(
                key.clone(),
                Arc::clone(offsets),
                format::index_file(index_root, source_path, revision),
            );
        }
        cache.finish_flight(&key, &flight, result.as_ref().err().cloned());

        return match result {
            Ok(offsets) => Ok((
                offsets,
                IndexState::Built,
                started.elapsed().as_millis() as u64,
            )),
            Err(error) => Err(error),
        };
    }
}

/// 建 / 载落盘索引并整表读入内存；落盘索引仍按窗口切片读，只有提升到缓存才整表读。
fn build_offsets(
    index_root: &Path,
    source_path: &Path,
    revision: &str,
    file_bytes: u64,
    explicit: Option<&'static Encoding>,
) -> Result<Arc<Vec<u64>>, String> {
    let mut loaded = match format::load(index_root, source_path, revision, file_bytes)? {
        Some(loaded) => loaded,
        None => {
            let built = format::build(source_path, revision, explicit)?;
            format::store(index_root, source_path, &built)?;
            format::load(index_root, source_path, revision, built.file_bytes)?.ok_or_else(|| {
                format!(
                    "'{}': index was written but could not be reloaded",
                    source_path.display()
                )
            })?
        }
    };
    Ok(Arc::new(loaded.all_offsets()?))
}

/// 换编码重建：UTF-16 的行边界与单字节切法不同，同一 `revision` 的索引内容随之改变，
/// 因此缓存项必须整体替换，否则下一次调用会拿到按旧编码切出的起点表。
pub(crate) fn rebuild_offsets(
    index_root: &Path,
    source_path: &Path,
    revision: &str,
    encoding: &'static Encoding,
) -> Result<Arc<Vec<u64>>, String> {
    let built = format::build(source_path, revision, Some(encoding))?;
    format::store(index_root, source_path, &built)?;
    let mut loaded = format::load(index_root, source_path, revision, built.file_bytes)?
        .ok_or_else(|| {
            format!(
                "'{}': index was rebuilt but could not be reloaded",
                source_path.display()
            )
        })?;
    let offsets = Arc::new(loaded.all_offsets()?);
    index_cache().insert(
        cache_key(index_root, source_path, revision),
        Arc::clone(&offsets),
        format::index_file(index_root, source_path, revision),
    );
    Ok(offsets)
}
