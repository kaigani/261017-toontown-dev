//! The window's drawing thumbnails, off the document thread (04 §12.3: "thumbnails are computed in the background with
//! a progress indication"; 04 §17.1 item 10: background thumbnail computation never blocks commands; M5 ruling E-56).
//!
//! Two paths. Agents keep the declared queries, synchronous with no pending state (04 §17.1 item 9:
//! `element.drawing.list {thumbnails}`, `xsheet.exposures.get {include: ["thumbnails"]}`). The window asks through a host
//! read, never a command: [`crate::ExecutorHandle::thumbnails_read`] takes, in one short call on the document thread,
//! each drawing's current revision and an immutable snapshot of the document (copy-on-write: no copy of the drawings),
//! the planar-map cache and the project's blobs; it answers what the cache holds for those revisions and sizes, and
//! queues the rest. One worker thread renders the queue from the snapshot ([`crate::commands::draw_render::
//! thumbnail_png`]: the render the queries make, from shared sources) into the cache, keyed by drawing, revision and
//! size and bounded in bytes, and announces each finished thumbnail with the host-only notice `shell.thumbnailsReady`
//! (with how many are still out, the window's progress line). A read replaces the queue: work for drawings the window no
//! longer shows (a scroll) is dropped. A drawing edited while its thumbnail renders has a new revision: the image of the
//! old one is never answered for it (the revision is part of the key and of the answer).
//!
//! The agents' derived `thumbnail` of a drawing (`doc.object.get {include: ["derived"]}`, 10 line 493: a blob) shares
//! the cache: it is rendered on the document thread when the cache does not hold it (synchronous, 04 §17.1 item 9), and
//! names the package's derived-cache entry [`entry_name`] (10 line 743, `cache/...`), which `doc.blob.get {entry}`
//! answers while the drawing is at that revision.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use serde_json::json;
use tt_model::Id;
use tt_model::document::Document;

/// The cache's bound in bytes (the PNGs it holds): a fixed share of the display caches (M5 ruling E-56; 01 §7.3's
/// `display.cacheMemoryPercent` governs the display caches as a whole).
pub const CACHE_BOUND: usize = 64 << 20;

/// The sizes a thumbnail may have (04 §12.3: small images in cells; the Cells panel's preview).
pub const MIN_SIDE: u32 = 16;
/// The largest thumbnail side.
pub const MAX_SIDE: u32 = 512;

/// The side of a drawing's derived `thumbnail` (the size `xsheet.exposures.get {include: ["thumbnails"]}` renders).
pub const DERIVED_SIDE: u32 = 64;

/// The folder of the derived thumbnails' entries (10 line 743: optional derived caches go under `cache/`).
const ENTRY_PREFIX: &str = "cache/thumbnails/";

/// The derived-cache entry that names a thumbnail: `cache/thumbnails/<drawing id>-<revision>-<side>.png`.
#[must_use]
pub fn entry_name(key: &ThumbnailKey) -> String {
    format!("{ENTRY_PREFIX}{}-{}-{}.png", key.drawing_id, key.rev, key.size)
}

/// The key an entry name of [`entry_name`]'s form names; `None` for any other entry.
#[must_use]
pub fn parse_entry(entry: &str) -> Option<ThumbnailKey> {
    let rest = entry.strip_prefix(ENTRY_PREFIX)?.strip_suffix(".png")?;
    let mut parts = rest.rsplitn(3, '-');
    let size = parts.next()?.parse::<u32>().ok()?;
    let rev = parts.next()?.parse::<u64>().ok()?;
    let drawing_id = parts.next()?.to_owned();
    (!drawing_id.is_empty() && (MIN_SIDE..=MAX_SIDE).contains(&size)).then_some(ThumbnailKey { drawing_id, rev, size })
}

/// One thumbnail the window asks for.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThumbnailRequest {
    /// The drawing.
    pub drawing_id: String,
    /// Its side in pixels (clamped to 16-512).
    pub size: u32,
}

/// A thumbnail's identity: the drawing at a revision, at a size.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailKey {
    /// The drawing.
    pub drawing_id: String,
    /// Its revision when the thumbnail was (or will be) rendered.
    pub rev: u64,
    /// The side in pixels.
    pub size: u32,
}

/// A thumbnail the cache holds.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailReady {
    /// Which thumbnail.
    #[serde(flatten)]
    pub key: ThumbnailKey,
    /// The PNG, base64.
    pub png: String,
}

/// The answer to a read: the thumbnails ready now, the ones queued (announced by `shell.thumbnailsReady` when done),
/// and the drawings that do not exist.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailRead {
    /// Ready now, at the drawings' current revisions.
    pub ready: Vec<ThumbnailReady>,
    /// Queued.
    pub pending: Vec<ThumbnailKey>,
    /// Asked for drawings the document does not have.
    pub missing: Vec<String>,
}

/// What a render needs, taken on the document thread: shared, immutable from then on.
#[derive(Clone)]
pub(crate) struct Snapshot {
    pub doc: Arc<Document>,
    pub planar: crate::planar::SharedCache,
    pub blobs: Option<Arc<tt_persist::container::Blobs>>,
}

struct Job {
    key: ThumbnailKey,
    snapshot: Snapshot,
}

#[derive(Default)]
struct State {
    /// PNG bytes by key, with the use counter of their last read (the least recently used go first).
    cache: BTreeMap<ThumbnailKey, (Arc<Vec<u8>>, u64)>,
    bytes: usize,
    uses: u64,
    queue: VecDeque<Job>,
    /// The key the worker renders now (a read asking for it again does not queue it twice).
    rendering: Option<ThumbnailKey>,
    worker: bool,
}

/// The window's thumbnail service (one per executor).
pub(crate) struct Service {
    state: Mutex<State>,
    wake: Condvar,
    bound: usize,
    bus: Arc<crate::host::Bus>,
}

impl std::fmt::Debug for Service {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("thumbnails::Service").field("bound", &self.bound).finish_non_exhaustive()
    }
}

impl Service {
    pub(crate) fn new(bus: Arc<crate::host::Bus>, bound: usize) -> Arc<Self> {
        Arc::new(Self { state: Mutex::new(State::default()), wake: Condvar::new(), bound, bus })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Answers a read from the cache and replaces the queue with its misses (see the module's documentation).
    pub(crate) fn read(self: &Arc<Self>, items: &[ThumbnailRequest], snapshot: &Snapshot) -> ThumbnailRead {
        let mut out = ThumbnailRead::default();
        let mut jobs = VecDeque::new();
        let mut st = self.lock();
        for it in items {
            let drawing = Id::parse(&it.drawing_id).ok().and_then(|id| snapshot.doc.drawing(&id));
            let Some(d) = drawing else {
                out.missing.push(it.drawing_id.clone());
                continue;
            };
            let key = ThumbnailKey {
                drawing_id: it.drawing_id.clone(),
                rev: *d.rev(),
                size: it.size.clamp(MIN_SIDE, MAX_SIDE),
            };
            st.uses += 1;
            let uses = st.uses;
            if let Some((png, used)) = st.cache.get_mut(&key) {
                *used = uses;
                out.ready.push(ThumbnailReady { key, png: tt_persist::b64::encode(png) });
                continue;
            }
            if !out.pending.contains(&key) {
                out.pending.push(key.clone());
                if st.rendering.as_ref() != Some(&key) {
                    jobs.push_back(Job { key, snapshot: snapshot.clone() });
                }
            }
        }
        // The queue is what the window shows now: work for anything else is dropped (a scroll).
        st.queue = jobs;
        if !st.queue.is_empty() && !st.worker {
            st.worker = true;
            let me = Arc::clone(self);
            let started = std::thread::Builder::new().name("tt-thumbnails".into()).spawn(move || me.work());
            if started.is_err() {
                st.worker = false;
            }
        }
        drop(st);
        self.wake.notify_one();
        out
    }

    /// The worker: renders the queue, one thumbnail at a time, into the cache.
    fn work(self: Arc<Self>) {
        loop {
            let job = {
                let mut st = self.lock();
                loop {
                    if let Some(j) = st.queue.pop_front() {
                        st.rendering = Some(j.key.clone());
                        break j;
                    }
                    // The executor gone (only this worker holds the service), the worker ends.
                    if Arc::strong_count(&self) == 1 {
                        st.worker = false;
                        return;
                    }
                    st = self
                        .wake
                        .wait_timeout(st, std::time::Duration::from_secs(5))
                        .unwrap_or_else(PoisonError::into_inner)
                        .0;
                }
            };
            #[cfg(feature = "test-hooks")]
            crate::hooks::thumbnail_delay();
            let png = render(&job);
            let left = {
                let mut st = self.lock();
                st.rendering = None;
                if let Some(png) = png {
                    st.uses += 1;
                    let uses = st.uses;
                    st.bytes += png.len();
                    if let Some((old, _)) = st.cache.insert(job.key.clone(), (Arc::new(png), uses)) {
                        st.bytes = st.bytes.saturating_sub(old.len());
                    }
                    self.evict(&mut st);
                }
                st.queue.len()
            };
            crate::broadcast_one(
                &self.bus,
                crate::session::Event {
                    name: "shell.thumbnailsReady".to_owned(),
                    payload: json!({ "items": [job.key], "pending": left }),
                },
            );
        }
    }

    /// Drops the least recently used thumbnails while the cache passes its bound.
    fn evict(&self, st: &mut State) {
        #[cfg(feature = "test-hooks")]
        let bound = crate::hooks::thumbnail_cache_bound().unwrap_or(self.bound);
        #[cfg(not(feature = "test-hooks"))]
        let bound = self.bound;
        while st.bytes > bound {
            let Some(oldest) = st.cache.iter().min_by_key(|(_, (_, used))| *used).map(|(k, _)| k.clone()) else {
                break;
            };
            if let Some((png, _)) = st.cache.remove(&oldest) {
                st.bytes = st.bytes.saturating_sub(png.len());
            }
        }
    }

    /// The thumbnail the cache holds for a key, marked as used.
    pub(crate) fn cached(&self, key: &ThumbnailKey) -> Option<Arc<Vec<u8>>> {
        let mut st = self.lock();
        st.uses += 1;
        let uses = st.uses;
        st.cache.get_mut(key).map(|(png, used)| {
            *used = uses;
            Arc::clone(png)
        })
    }

    /// Keeps a thumbnail rendered elsewhere (the agents' synchronous path) in the cache, within its bound.
    pub(crate) fn keep(&self, key: ThumbnailKey, png: Arc<Vec<u8>>) {
        let mut st = self.lock();
        st.uses += 1;
        let uses = st.uses;
        st.bytes += png.len();
        if let Some((old, _)) = st.cache.insert(key, (png, uses)) {
            st.bytes = st.bytes.saturating_sub(old.len());
        }
        self.evict(&mut st);
    }

    /// The cache's size in bytes and its number of thumbnails.
    pub(crate) fn usage(&self) -> (usize, usize) {
        let st = self.lock();
        (st.bytes, st.cache.len())
    }
}

/// Renders one queued thumbnail from its snapshot; `None` when the drawing cannot be rendered (it is not answered).
fn render(job: &Job) -> Option<Vec<u8>> {
    let id = Id::parse(&job.key.drawing_id).ok()?;
    let d = job.snapshot.doc.drawing(&id)?;
    let never = || false;
    let src = crate::commands::draw_render::Sources {
        cache: &job.snapshot.planar,
        blobs: job.snapshot.blobs.as_deref(),
        selection: None,
        stop: &never,
    };
    crate::commands::draw_render::thumbnail_png(&src, &job.snapshot.doc, d, job.key.size).ok()
}
