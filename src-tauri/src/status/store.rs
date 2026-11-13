use crate::status::domain::{self, Document, Entry, Point};
use chrono::Utc;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

pub const MAX_STATUS_BYTES: usize = 1024 * 1024;
#[derive(Debug)]
pub enum StatusError {
    InvalidInput(String),
    InvalidDocument(String),
    Io(String),
    Conflict(String),
    LockTimeout(String),
}
impl std::fmt::Display for StatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(s)
            | Self::InvalidDocument(s)
            | Self::Io(s)
            | Self::Conflict(s)
            | Self::LockTimeout(s) => f.write_str(s),
        }
    }
}
impl From<String> for StatusError {
    fn from(value: String) -> Self {
        Self::InvalidDocument(value)
    }
}
impl From<std::io::Error> for StatusError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error.to_string())
    }
}

const MAX_ARCHIVE_BYTES: usize = 5 * 1024 * 1024;
const MAX_PRIVATE_BYTES: usize = 10 * 1024 * 1024;
const ARCHIVE_DAYS: i64 = 30;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RemovedPoint {
    pub id: String,
    pub entry: Entry,
    pub point_index: usize,
    pub removed_at: String,
    pub reason: String,
}

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrivateState {
    version: u32,
    pins: Vec<Entry>,
    archive: Vec<RemovedPoint>,
    pending: Option<PendingWrite>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingWrite {
    before_raw: Option<String>,
    after_raw: String,
    after_pins: Vec<Entry>,
}

fn trim_archive(archive: &mut Vec<RemovedPoint>) -> Result<(), StatusError> {
    let cutoff = Utc::now() - chrono::Duration::days(ARCHIVE_DAYS);
    archive.retain(|item| {
        chrono::DateTime::parse_from_rfc3339(&item.removed_at).is_ok_and(|at| at >= cutoff)
    });
    archive.sort_by(|a, b| a.removed_at.cmp(&b.removed_at).then(a.id.cmp(&b.id)));
    while serde_json::to_vec(archive)
        .map_err(|error| StatusError::Io(error.to_string()))?
        .len()
        > MAX_ARCHIVE_BYTES
    {
        archive.remove(0);
    }
    Ok(())
}

fn overlay_pins(document: &mut Document, pins: &[Entry]) {
    for saved in pins {
        let matching = document.entries.iter().position(|entry| {
            entry.folder_name == saved.folder_name
                && entry.session_id == saved.session_id
                && entry.branch == saved.branch
        });
        if let Some(index) = matching {
            let entry = &mut document.entries[index];
            for pin in &saved.points {
                entry.points.retain(|point| point.id != pin.id);
                entry.points.push(pin.clone());
            }
        } else {
            let mut restored = saved.clone();
            if restored.session_id.is_some()
                && document.entries.iter().any(|entry| {
                    entry.folder_name == restored.folder_name
                        && entry.session_id == restored.session_id
                })
            {
                restored.session_id = None;
            }
            document.entries.push(restored);
        }
    }
}

pub struct StatusStore {
    path: PathBuf,
}
struct SidecarLock {
    file: File,
}
impl Drop for SidecarLock {
    fn drop(&mut self) {
        let _ = self.file.unlock();
        // Keep the inode stable for writers that already opened the sidecar.
    }
}
impl StatusStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
    fn parent(&self) -> Result<&std::path::Path, StatusError> {
        self.path
            .parent()
            .ok_or_else(|| StatusError::Io("Status directory unavailable".into()))
    }
    fn lock(&self) -> Result<SidecarLock, StatusError> {
        self.lock_for(Duration::from_secs(2))
    }
    fn lock_for(&self, timeout: Duration) -> Result<SidecarLock, StatusError> {
        fs::create_dir_all(self.parent()?)?;
        let path = self.path.with_extension("lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)?;
        let deadline = Instant::now() + timeout;
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(SidecarLock { file }),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        return Err(StatusError::LockTimeout("Status file is busy. Retry shortly; another process is holding its lock.".into()));
                    }
                    thread::sleep(Duration::from_millis(10))
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    fn read_raw_document(&self) -> Result<(Document, bool), StatusError> {
        let file = match File::open(&self.path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok((Document::default(), false))
            }
            Err(error) => return Err(error.into()),
        };
        let mut bytes = Vec::with_capacity(MAX_STATUS_BYTES.min(8192));
        file.take((MAX_STATUS_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_STATUS_BYTES {
            return Err(StatusError::InvalidDocument(
                "Status JSON exceeds the 1 MiB limit".into(),
            ));
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|error| {
            StatusError::InvalidDocument(format!("Invalid status JSON: {error}"))
        })?;
        let inline_pins = value
            .get("entries")
            .and_then(|v| v.as_array())
            .is_some_and(|entries| {
                entries.iter().any(|entry| {
                    entry
                        .get("points")
                        .and_then(|v| v.as_array())
                        .is_some_and(|points| {
                            points.iter().any(|point| point.get("pinned").is_some())
                        })
                })
            });
        let mut document: Document = serde_json::from_value(value).map_err(|error| {
            StatusError::InvalidDocument(format!("Invalid status JSON: {error}"))
        })?;
        domain::validate(&document).map_err(StatusError::InvalidDocument)?;
        if document.version == 1 {
            domain::assign_legacy_ids(&mut document);
            domain::validate(&document).map_err(StatusError::InvalidDocument)?;
        }
        Ok((document, inline_pins))
    }
    fn read_unlocked(&self) -> Result<Document, StatusError> {
        let private = self.recover_private()?;
        let (mut document, inline_pins) = self.read_raw_document()?;
        overlay_pins(&mut document, &private.pins);
        domain::validate(&document).map_err(StatusError::InvalidDocument)?;
        if inline_pins {
            self.save_unlocked(&document)?;
        }
        Ok(document)
    }
    pub fn read(&self) -> Result<Document, StatusError> {
        let _lock = self.lock()?;
        self.read_unlocked()
    }
    pub fn read_project(&self, folder: &str, session: &str) -> Result<Vec<Entry>, StatusError> {
        if !domain::single_line(folder) || !domain::single_line(session) {
            return Err(StatusError::InvalidInput(
                "Project folder and session ID must be non-empty single lines".into(),
            ));
        }
        Ok(self
            .read()?
            .entries
            .into_iter()
            .filter(|entry| {
                entry.folder_name == folder && entry.session_id.as_deref() == Some(session)
            })
            .map(|mut entry| {
                entry.points.retain(|point| !point.pinned);
                entry
            })
            .filter(|entry| !entry.points.is_empty())
            .collect())
    }
    fn private_path(&self) -> PathBuf {
        self.path.with_extension("private.json")
    }
    fn read_private(&self) -> Result<PrivateState, StatusError> {
        let bytes = match File::open(self.private_path()) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take((MAX_PRIVATE_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)?;
                bytes
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(PrivateState {
                    version: 1,
                    ..Default::default()
                });
            }
            Err(error) => return Err(error.into()),
        };
        if bytes.len() > MAX_PRIVATE_BYTES {
            return Err(StatusError::InvalidDocument(
                "Private status data exceeds its size limit".into(),
            ));
        }
        let private: PrivateState = serde_json::from_slice(&bytes).map_err(|error| {
            StatusError::InvalidDocument(format!("Invalid private status data: {error}"))
        })?;
        if private.version != 1 {
            return Err(StatusError::InvalidDocument(
                "Unsupported private status version; original data was preserved".into(),
            ));
        }
        domain::validate(&Document {
            version: 2,
            entries: private.pins.clone(),
        })
        .map_err(StatusError::InvalidDocument)?;
        if private
            .pins
            .iter()
            .any(|entry| entry.points.iter().any(|point| !point.pinned))
        {
            return Err(StatusError::InvalidDocument(
                "Private pin data contains an unpinned point".into(),
            ));
        }
        for item in &private.archive {
            if item.entry.points.len() != 1
                || chrono::DateTime::parse_from_rfc3339(&item.removed_at).is_err()
            {
                return Err(StatusError::InvalidDocument(
                    "Invalid removed-item record".into(),
                ));
            }
            domain::validate(&Document {
                version: 2,
                entries: vec![item.entry.clone()],
            })
            .map_err(StatusError::InvalidDocument)?;
        }
        if let Some(pending) = &private.pending {
            domain::validate(&Document {
                version: 2,
                entries: pending.after_pins.clone(),
            })
            .map_err(StatusError::InvalidDocument)?;
            let after: Document = serde_json::from_str(&pending.after_raw).map_err(|error| {
                StatusError::InvalidDocument(format!("Invalid status recovery journal: {error}"))
            })?;
            domain::validate(&after).map_err(StatusError::InvalidDocument)?;
        }
        Ok(private)
    }
    fn persist_bytes(&self, path: &std::path::Path, bytes: &[u8]) -> Result<(), StatusError> {
        let mut temp = tempfile::NamedTempFile::new_in(self.parent()?)?;
        temp.write_all(bytes)?;
        temp.as_file().sync_all()?;
        temp.persist(path)
            .map_err(|error| StatusError::Io(error.to_string()))?;
        Ok(())
    }
    fn save_private(&self, private: &PrivateState) -> Result<(), StatusError> {
        let bytes = serde_json::to_vec(private)
            .map_err(|error| StatusError::InvalidDocument(error.to_string()))?;
        if bytes.len() > MAX_PRIVATE_BYTES {
            return Err(StatusError::InvalidDocument(
                "Private status data exceeds its size limit; no points were deleted".into(),
            ));
        }
        self.persist_bytes(&self.private_path(), &bytes)
    }
    fn raw_bytes(&self) -> Result<Option<Vec<u8>>, StatusError> {
        match File::open(&self.path) {
            Ok(file) => {
                let mut bytes = Vec::new();
                file.take((MAX_STATUS_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)?;
                if bytes.len() > MAX_STATUS_BYTES {
                    return Err(StatusError::InvalidDocument(
                        "Status JSON exceeds the 1 MiB limit".into(),
                    ));
                }
                Ok(Some(bytes))
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
    fn recover_private(&self) -> Result<PrivateState, StatusError> {
        let mut private = self.read_private()?;
        if let Some(pending) = private.pending.take() {
            let raw = self.raw_bytes()?;
            if raw.as_deref() == Some(pending.after_raw.as_bytes()) {
                private.pins = pending.after_pins;
            } else if raw.as_deref() == pending.before_raw.as_deref().map(str::as_bytes) {
                self.persist_bytes(&self.path, pending.after_raw.as_bytes())?;
                private.pins = pending.after_pins;
            } else {
                // The raw commit may already have removed a newly pinned point.
                // Retain both acknowledged pins and newly pending pins before
                // discarding the journal, without overwriting newer legacy bytes.
                for mut saved in pending.after_pins {
                    saved.points.retain(|point| {
                        !private.pins.iter().any(|entry| {
                            entry.folder_name == saved.folder_name
                                && entry.branch == saved.branch
                                && (entry.session_id == saved.session_id
                                    || entry.session_id.is_none()
                                    || saved.session_id.is_none())
                                && entry.points.iter().any(|existing| existing.id == point.id)
                        })
                    });
                    if saved.points.is_empty() {
                        continue;
                    }
                    if let Some(entry) = private.pins.iter_mut().find(|entry| {
                        entry.folder_name == saved.folder_name
                            && entry.session_id == saved.session_id
                            && entry.branch == saved.branch
                    }) {
                        entry.points.extend(saved.points);
                    } else {
                        private.pins.push(saved);
                    }
                }
                // Pending pins can belong to a newer branch for the same
                // session. Preserve old-branch pins as anonymous user content.
                let mut sessions = std::collections::HashSet::new();
                for entry in private.pins.iter_mut().rev() {
                    if let Some(session) = &entry.session_id {
                        if !sessions.insert((entry.folder_name.clone(), session.clone())) {
                            entry.session_id = None;
                        }
                    }
                }
            }
            self.save_private(&private)?;
        }
        Ok(private)
    }
    fn save_unlocked(&self, document: &Document) -> Result<usize, StatusError> {
        let mut raw_document = document.clone();
        let mut pins = Vec::new();
        for entry in &mut raw_document.entries {
            let mut pinned = entry.clone();
            pinned.points.retain(|point| point.pinned);
            if !pinned.points.is_empty() {
                pins.push(pinned);
            }
            entry.points.retain(|point| !point.pinned);
        }
        raw_document
            .entries
            .retain(|entry| !entry.points.is_empty());
        let mut value = serde_json::to_value(&raw_document)
            .map_err(|error| StatusError::InvalidDocument(error.to_string()))?;
        for entry in value["entries"].as_array_mut().unwrap() {
            for point in entry["points"].as_array_mut().unwrap() {
                point.as_object_mut().unwrap().remove("pinned");
            }
        }
        let bytes = serde_json::to_vec_pretty(&value)
            .map_err(|error| StatusError::InvalidDocument(error.to_string()))?;
        if bytes.len() > MAX_STATUS_BYTES
            || serde_json::to_vec(&pins).unwrap().len() > MAX_STATUS_BYTES
        {
            return Err(StatusError::InvalidDocument(
                "Status or pinned data exceeds the 1 MiB limit".into(),
            ));
        }
        let mut private = self.recover_private()?;
        let before_raw = self.raw_bytes()?;
        // Inline pins are an earlier application format. Preserve exact original
        // bytes before the first conversion to the legacy-compatible wire file.
        if before_raw.as_ref().is_some_and(|raw| {
            serde_json::from_slice::<serde_json::Value>(raw).is_ok_and(|doc| {
                doc.get("entries")
                    .and_then(|v| v.as_array())
                    .is_some_and(|entries| {
                        entries.iter().any(|entry| {
                            entry
                                .get("points")
                                .and_then(|v| v.as_array())
                                .is_some_and(|points| {
                                    points.iter().any(|point| point.get("pinned").is_some())
                                })
                        })
                    })
            })
        }) {
            let backup = self
                .path
                .with_extension(format!("pre-pins-{}.backup", uuid::Uuid::new_v4()));
            self.persist_bytes(&backup, before_raw.as_deref().unwrap())?;
        }
        if private.pins == pins {
            self.persist_bytes(&self.path, &bytes)?;
        } else {
            private.pending = Some(PendingWrite {
                before_raw: before_raw
                    .map(|raw| {
                        String::from_utf8(raw)
                            .map_err(|error| StatusError::InvalidDocument(error.to_string()))
                    })
                    .transpose()?,
                after_raw: String::from_utf8(bytes.clone()).unwrap(),
                after_pins: pins.clone(),
            });
            self.save_private(&private)?;
            self.persist_bytes(&self.path, &bytes)?;
            private.pins = pins;
            private.pending = None;
            self.save_private(&private)?;
        }
        Ok(bytes.len())
    }
    pub fn upsert(
        &self,
        folder: String,
        session: String,
        branch: String,
        points: Vec<Point>,
    ) -> Result<usize, StatusError> {
        self.upsert_with_repository(folder, session, branch, points, None)
    }
    pub fn upsert_with_repository(
        &self,
        folder: String,
        session: String,
        branch: String,
        points: Vec<Point>,
        repository: Option<String>,
    ) -> Result<usize, StatusError> {
        self.upsert_with_repository_result(folder, session, branch, points, repository)
            .map(|(bytes, _)| bytes)
    }
    pub fn upsert_with_repository_result(
        &self,
        folder: String,
        session: String,
        branch: String,
        points: Vec<Point>,
        repository: Option<String>,
    ) -> Result<(usize, Vec<Point>), StatusError> {
        let _lock = self.lock()?;
        if !domain::single_line(&folder)
            || !domain::single_line(&session)
            || !domain::single_line(&branch)
        {
            return Err(StatusError::InvalidInput(
                "Folder, session ID, and branch must be non-empty single lines".into(),
            ));
        }
        let mut document = self.read_unlocked()?;
        let repository = repository.or_else(|| {
            document
                .entries
                .iter()
                .find(|e| e.folder_name == folder && e.session_id.as_deref() == Some(&session))
                .and_then(|e| e.repository.clone())
        });
        let identity = (folder.clone(), session.clone());
        domain::upsert(&mut document, folder, session, branch, points, Utc::now())
            .map_err(StatusError::InvalidInput)?;
        if let Some(entry) = document
            .entries
            .iter_mut()
            .find(|e| e.folder_name == identity.0 && e.session_id.as_deref() == Some(&identity.1))
        {
            entry.repository = repository;
        }
        domain::validate(&document).map_err(StatusError::InvalidDocument)?;
        let points = document
            .entries
            .iter()
            .find(|entry| {
                entry.folder_name == identity.0 && entry.session_id.as_deref() == Some(&identity.1)
            })
            .map(|entry| {
                entry
                    .points
                    .iter()
                    .filter(|point| !point.pinned)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default();
        let bytes = self.save_unlocked(&document)?;
        Ok((bytes, points))
    }
    pub fn set_point_pinned(
        &self,
        folder: &str,
        session: Option<&str>,
        branch: &str,
        updated_at: &str,
        point_id: &str,
        pinned: bool,
    ) -> Result<(), StatusError> {
        let _lock = self.lock()?;
        let mut document = self.read_unlocked()?;
        let point = document
            .entries
            .iter_mut()
            .find(|e| {
                e.folder_name == folder
                    && e.session_id.as_deref() == session
                    && e.branch == branch
                    && e.updated_at == updated_at
                    && e.points.iter().any(|p| p.id.as_deref() == Some(point_id))
            })
            .and_then(|e| {
                e.points
                    .iter_mut()
                    .find(|p| p.id.as_deref() == Some(point_id))
            })
            .ok_or_else(|| StatusError::Conflict("This point changed. Please try again.".into()))?;
        point.pinned = pinned;
        document.version = 2;
        domain::validate(&document).map_err(StatusError::InvalidDocument)?;
        self.save_unlocked(&document)?;
        Ok(())
    }
    pub fn dismiss(
        &self,
        folder: &str,
        session: Option<&str>,
        branch: &str,
        updated_at: &str,
        point_id: &str,
        point_index: Option<usize>,
    ) -> Result<bool, StatusError> {
        let _lock = self.lock()?;
        let mut document = self.read_unlocked()?;
        if !domain::dismiss(
            &mut document,
            folder,
            session,
            branch,
            updated_at,
            point_id,
            point_index,
        ) {
            return Ok(false);
        }
        domain::validate(&document).map_err(StatusError::InvalidDocument)?;
        self.save_unlocked(&document)?;
        Ok(true)
    }
    /// Merge one dismissed point without replacing newer agent submissions.
    pub fn restore(&self, saved: Entry, index: usize) -> Result<(), StatusError> {
        let _lock = self.lock()?;
        self.restore_unlocked(saved, index)
    }
    fn restore_unlocked(&self, mut saved: Entry, index: usize) -> Result<(), StatusError> {
        let mut document = self.read_unlocked()?;
        if saved.points.len() != 1 {
            return Err(StatusError::InvalidInput(
                "Undo requires exactly one point".into(),
            ));
        }
        let point = saved.points.remove(0);
        if let Some(entry) = document.entries.iter_mut().find(|entry| {
            entry.folder_name == saved.folder_name
                && entry.session_id == saved.session_id
                && (saved.session_id.is_some() || entry.branch == saved.branch)
        }) {
            if entry.branch != saved.branch || entry.points.iter().any(|p| p.id == point.id) {
                return Err(StatusError::Conflict(
                    "This point has changed or already exists.".into(),
                ));
            }
            entry.points.insert(index.min(entry.points.len()), point);
        } else {
            saved.points.push(point);
            document.entries.push(saved);
        }
        document.version = 2;
        domain::validate(&document).map_err(StatusError::InvalidDocument)?;
        self.save_unlocked(&document)?;
        Ok(())
    }
    pub fn preview_retention(&self, rules: &[domain::RetentionRule]) -> Result<usize, StatusError> {
        let _lock = self.lock()?;
        let mut document = self.read_unlocked()?;
        let before = document
            .entries
            .iter()
            .map(|entry| entry.points.len())
            .sum::<usize>();
        domain::apply_retention(&mut document, rules, Utc::now())
            .map_err(StatusError::InvalidInput)?;
        Ok(before
            - document
                .entries
                .iter()
                .map(|entry| entry.points.len())
                .sum::<usize>())
    }
    pub fn list_removed(&self) -> Result<Vec<RemovedPoint>, StatusError> {
        let _lock = self.lock()?;
        let mut private = self.recover_private()?;
        trim_archive(&mut private.archive)?;
        self.save_private(&private)?;
        private.archive.reverse();
        Ok(private.archive)
    }
    pub fn restore_removed(&self, id: &str) -> Result<(), StatusError> {
        let _lock = self.lock()?;
        let private = self.recover_private()?;
        let item = private
            .archive
            .iter()
            .find(|item| item.id == id)
            .ok_or_else(|| {
                StatusError::Conflict("This removed item is no longer available".into())
            })?;
        let mut saved = item.entry.clone();
        // Restored user-owned content must not immediately expire again.
        saved.points[0].pinned = true;
        let document = self.read_unlocked()?;
        if saved.session_id.is_some()
            && document.entries.iter().any(|entry| {
                entry.folder_name == saved.folder_name
                    && entry.session_id == saved.session_id
                    && entry.branch != saved.branch
            })
        {
            saved.session_id = None;
        }
        let already_restored = document.entries.iter().any(|entry| {
            entry.folder_name == saved.folder_name
                && entry.session_id == saved.session_id
                && entry.branch == saved.branch
                && entry.points.iter().any(|point| point == &saved.points[0])
        });
        if !already_restored {
            self.restore_unlocked(saved, item.point_index)?;
        }
        let mut private = self.recover_private()?;
        private.archive.retain(|item| item.id != id);
        self.save_private(&private)
    }
    pub fn apply_retention(&self, rules: &[domain::RetentionRule]) -> Result<bool, StatusError> {
        let _lock = self.lock()?;
        let mut document = self.read_unlocked()?;
        let before = document.clone();
        let now = Utc::now();
        let changed = domain::apply_retention(&mut document, rules, now)
            .map_err(StatusError::InvalidInput)?;
        if changed {
            let mut private = self.recover_private()?;
            for entry in before.entries {
                for (index, point) in entry.points.iter().enumerate() {
                    let kept = document.entries.iter().any(|current| {
                        current.folder_name == entry.folder_name
                            && current.session_id == entry.session_id
                            && current.branch == entry.branch
                            && current
                                .points
                                .iter()
                                .any(|candidate| candidate.id == point.id)
                    });
                    if kept {
                        continue;
                    }
                    let identity = serde_json::to_vec(&(
                        &entry.folder_name,
                        &entry.session_id,
                        &entry.branch,
                        &point.id,
                        &point.submitted_at,
                        &point.text,
                    ))
                    .unwrap();
                    let id = uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_OID, &identity).to_string();
                    if private.archive.iter().any(|item| item.id == id) {
                        continue;
                    }
                    let time_expired = rules.iter().any(|rule| match rule {
                        domain::RetentionRule::Age(age) => point
                            .submitted_at
                            .as_deref()
                            .or(Some(entry.updated_at.as_str()))
                            .and_then(|at| chrono::DateTime::parse_from_rfc3339(at).ok())
                            .is_some_and(|at| {
                                now.signed_duration_since(at)
                                    .to_std()
                                    .is_ok_and(|elapsed| elapsed > *age)
                            }),
                        _ => false,
                    });
                    let mut saved = entry.clone();
                    saved.points = vec![point.clone()];
                    private.archive.push(RemovedPoint {
                        id,
                        entry: saved,
                        point_index: index,
                        removed_at: now.to_rfc3339(),
                        reason: if time_expired {
                            "Time-based deletion"
                        } else {
                            "Count-based deletion"
                        }
                        .into(),
                    });
                }
            }
            trim_archive(&mut private.archive)?;
            // This commit precedes deletion. A failed status write can be retried
            // without duplicating archive records, using the stable identity above.
            self.save_private(&private)?;
            self.save_unlocked(&document)?;
        }
        Ok(changed)
    }
    pub fn prune(&self, age: Duration) -> Result<bool, StatusError> {
        self.apply_retention(&[domain::RetentionRule::Age(age)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn point(text: &str) -> Point {
        Point {
            text: text.into(),
            done: false,
            pinned: false,
            link: None,
            submitted_at: None,
            id: None,
        }
    }

    #[test]
    fn pins_are_private_protected_and_survive_branch_changes() {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        store
            .upsert(
                "p".into(),
                "s".into(),
                "main".into(),
                vec![point("keep"), point("editable")],
            )
            .unwrap();
        let entry = store.read().unwrap().entries.remove(0);
        let id = entry.points[0].id.as_deref().unwrap();
        store
            .set_point_pinned("p", Some("s"), "main", &entry.updated_at, id, true)
            .unwrap();
        assert_eq!(store.read_project("p", "s").unwrap()[0].points.len(), 1);
        assert!(!store
            .dismiss("p", Some("s"), "main", &entry.updated_at, id, None)
            .unwrap());
        let mut attempted_edit = entry.points[0].clone();
        attempted_edit.text = "tampered".into();
        store
            .upsert(
                "p".into(),
                "s".into(),
                "main".into(),
                vec![attempted_edit, point("new")],
            )
            .unwrap();
        let current = store.read().unwrap().entries.remove(0);
        assert!(current.points.iter().any(|p| p.text == "keep" && p.pinned));
        assert!(!current.points.iter().any(|p| p.text == "tampered"));
        store
            .apply_retention(&[
                domain::RetentionRule::CountPerProject(0),
                domain::RetentionRule::Age(Duration::ZERO),
            ])
            .unwrap();
        assert!(store.read_project("p", "s").unwrap().is_empty());
        store
            .upsert(
                "p".into(),
                "s".into(),
                "feature".into(),
                vec![point("next")],
            )
            .unwrap();
        let doc = store.read().unwrap();
        let pinned = doc.entries.iter().find(|e| e.branch == "main").unwrap();
        assert_eq!(pinned.points[0].text, "keep");
        assert!(pinned.points[0].pinned);
        assert_eq!(
            store.read_project("p", "s").unwrap()[0].points[0].text,
            "next"
        );
        store
            .set_point_pinned(
                "p",
                None,
                "main",
                &pinned.updated_at,
                pinned.points[0].id.as_deref().unwrap(),
                false,
            )
            .unwrap();
        assert!(store
            .dismiss(
                "p",
                None,
                "main",
                &pinned.updated_at,
                pinned.points[0].id.as_deref().unwrap(),
                None
            )
            .unwrap());
    }

    #[test]
    fn undo_merges_newer_points_and_recreates_empty_entry() {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        store
            .upsert("p".into(), "s".into(), "main".into(), vec![point("old")])
            .unwrap();
        let saved = store.read().unwrap().entries.remove(0);
        let id = saved.points[0].id.as_deref().unwrap();
        assert!(store
            .dismiss("p", Some("s"), "main", &saved.updated_at, id, None)
            .unwrap());
        store.restore(saved.clone(), 0).unwrap();
        assert_eq!(store.read().unwrap().entries[0], saved);
        assert!(store.restore(saved.clone(), 0).is_err());
        assert!(store
            .dismiss("p", Some("s"), "main", &saved.updated_at, id, None)
            .unwrap());
        store
            .upsert("p".into(), "s".into(), "main".into(), vec![point("new")])
            .unwrap();
        store.restore(saved.clone(), 0).unwrap();
        let current = store.read().unwrap().entries.remove(0);
        assert_eq!(current.points[0], saved.points[0]);
        assert_eq!(current.points[1].text, "new");
    }

    #[test]
    fn migrates_v1_and_keeps_duplicate_ids_stable_across_edits() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        let old = serde_json::json!({"version":1,"entries":[{"folder_name":"p","session_id":"s","branch":"main","points":[{"text":"same"},{"text":"same"}],"updated_at":"2025-01-01T00:00:00Z"}]});
        fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
        let store = StatusStore::new(path.clone());
        let first = store.read().unwrap().entries[0]
            .points
            .iter()
            .map(|p| p.id.clone())
            .collect::<Vec<_>>();
        store
            .upsert(
                "p".into(),
                "s".into(),
                "main".into(),
                vec![point("same"), point("changed"), point("same")],
            )
            .unwrap();
        let second = store.read().unwrap().entries[0].points.clone();
        assert_eq!(first[0], second[0].id);
        assert_eq!(first[1], second[2].id);
        assert_eq!(second[0].submitted_at, second[2].submitted_at);
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(path).unwrap()).unwrap()
                ["version"],
            2
        );
    }
    #[test]
    fn invalid_input_does_not_touch_file_and_writers_keep_all_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        let store = StatusStore::new(path.clone());
        store
            .upsert("p".into(), "a".into(), "main".into(), vec![point("old")])
            .unwrap();
        let before = fs::read(&path).unwrap();
        assert!(store
            .upsert("".into(), "a".into(), "main".into(), vec![point("bad")])
            .is_err());
        assert_eq!(fs::read(&path).unwrap(), before);
        std::thread::scope(|scope| {
            for i in 0..8 {
                let path = &path;
                scope.spawn(move || {
                    StatusStore::new(path.clone())
                        .upsert("p".into(), format!("{i}"), "main".into(), vec![point("x")])
                        .unwrap();
                });
            }
        });
        assert_eq!(store.read().unwrap().entries.len(), 9);
    }

    #[test]
    fn stale_lock_file_does_not_block_writes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        fs::write(path.with_extension("lock"), []).unwrap();

        StatusStore::new(path.clone())
            .upsert("p".into(), "s".into(), "main".into(), vec![point("x")])
            .unwrap();

        assert_eq!(StatusStore::new(path).read().unwrap().entries.len(), 1);
        assert!(dir.path().join("status.lock").exists());
    }

    #[test]
    fn waiting_and_new_writers_share_the_same_lock_after_release() {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        let first = store.lock().unwrap();
        let path = dir.path().join("status.lock");
        // A waiting writer opens the inode before the current owner releases it.
        let waiting = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        assert_eq!(
            waiting.try_lock_exclusive().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        drop(first);
        waiting.try_lock_exclusive().unwrap();
        let newcomer = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .unwrap();
        assert_eq!(
            newcomer.try_lock_exclusive().unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
        FileExt::unlock(&waiting).unwrap();
        newcomer.try_lock_exclusive().unwrap();
    }

    #[test]
    fn invalid_updates_matrix_keeps_saved_bytes_unchanged() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        let store = StatusStore::new(path.clone());
        store
            .upsert(
                "project".into(),
                "session".into(),
                "main".into(),
                vec![point("original")],
            )
            .unwrap();
        let before = fs::read(&path).unwrap();
        for (session, text, link) in [
            ("", "text", None),
            ("session", " ", None),
            ("session", "text", Some("javascript:alert(1)")),
            ("session", "text", Some("https://")),
        ] {
            assert!(store
                .upsert(
                    "project".into(),
                    session.into(),
                    "main".into(),
                    vec![Point {
                        text: text.into(),
                        done: false,
                        pinned: false,
                        link: link.map(str::to_owned),
                        submitted_at: None,
                        id: None
                    }]
                )
                .is_err());
        }
        assert!(store
            .upsert("project".into(), "session".into(), "main".into(), vec![])
            .is_err());
        assert_eq!(fs::read(path).unwrap(), before);
    }
    #[test]
    fn ids_are_preserved_when_callers_return_them() {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        store
            .upsert("p".into(), "s".into(), "main".into(), vec![point("x")])
            .unwrap();
        let mut value = store.read().unwrap().entries[0].points[0].clone();
        let id = value.id.clone();
        value.text = "edited".into();
        store
            .upsert("p".into(), "s".into(), "main".into(), vec![value])
            .unwrap();
        assert_eq!(
            store.read().unwrap().entries[0].points[0].id.as_deref(),
            id.as_deref()
        );
    }
    #[test]
    fn parallel_updates_are_serialized_by_the_sidecar_lock() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        std::thread::scope(|scope| {
            for i in 0..24 {
                let path = &path;
                scope.spawn(move || {
                    StatusStore::new(path.clone())
                        .upsert("p".into(), format!("s{i}"), "main".into(), vec![point("x")])
                        .unwrap();
                });
            }
        });
        assert_eq!(StatusStore::new(path).read().unwrap().entries.len(), 24);
    }
    #[test]
    fn scoped_reads_exclude_other_sessions_and_reject_empty_identity() {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        store
            .upsert("p".into(), "a".into(), "main".into(), vec![point("a")])
            .unwrap();
        assert_eq!(store.read_project("p", "a").unwrap().len(), 1);
        assert!(store.read_project("p", "").is_err());
    }
    #[test]
    fn anonymous_legacy_records_remain_readable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        fs::write(&path, r#"{"version":1,"entries":[{"folder_name":"p","session_id":null,"branch":"main","points":[{"text":"old"}],"updated_at":"2025-01-01T00:00:00Z"}]}"#).unwrap();
        assert_eq!(
            StatusStore::new(path).read().unwrap().entries[0].session_id,
            None
        );
    }
    #[test]
    fn branch_changes_preserve_other_entries() {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        store
            .upsert("p".into(), "a".into(), "main".into(), vec![point("a")])
            .unwrap();
        store
            .upsert("q".into(), "a".into(), "dev".into(), vec![point("q")])
            .unwrap();
        store
            .upsert("p".into(), "a".into(), "feature".into(), vec![point("new")])
            .unwrap();
        assert_eq!(store.read().unwrap().entries.len(), 2);
    }
    #[test]
    fn unchanged_timestamp_is_retained() {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        store
            .upsert("p".into(), "a".into(), "main".into(), vec![point("a")])
            .unwrap();
        let before = store.read().unwrap().entries[0].points[0]
            .submitted_at
            .clone();
        store
            .upsert("p".into(), "a".into(), "main".into(), vec![point("a")])
            .unwrap();
        assert_eq!(
            store.read().unwrap().entries[0].points[0].submitted_at,
            before
        );
    }
    #[test]
    fn dismiss_requires_the_snapshot_guard() {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        store
            .upsert("p".into(), "a".into(), "main".into(), vec![point("a")])
            .unwrap();
        let entry = store.read().unwrap().entries.remove(0);
        assert!(!store
            .dismiss(
                "p",
                Some("a"),
                "main",
                "wrong",
                entry.points[0].id.as_deref().unwrap(),
                Some(0)
            )
            .unwrap());
    }

    #[test]
    fn mismatched_id_never_falls_back_to_the_supplied_index() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        let store = StatusStore::new(path.clone());
        store
            .upsert(
                "p".into(),
                "a".into(),
                "main".into(),
                vec![point("a"), point("b")],
            )
            .unwrap();
        let entry = store.read().unwrap().entries[0].clone();
        let before = fs::read(&path).unwrap();
        assert!(!store
            .dismiss(
                "p",
                Some("a"),
                "main",
                &entry.updated_at,
                "missing",
                Some(0)
            )
            .unwrap());
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(store
            .dismiss("p", Some("a"), "main", &entry.updated_at, "", Some(0))
            .unwrap());
    }

    #[test]
    fn explicit_id_edit_preserves_old_age_and_unknown_ids_are_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        store
            .upsert(
                "p".into(),
                "a".into(),
                "main".into(),
                vec![point("a"), point("b")],
            )
            .unwrap();
        let old = store.read().unwrap().entries[0].points.clone();
        let mut edited = old[0].clone();
        edited.text = "changed".into();
        edited.submitted_at = None;
        store
            .upsert(
                "p".into(),
                "a".into(),
                "main".into(),
                vec![edited, old[1].clone()],
            )
            .unwrap();
        let next = store.read().unwrap().entries[0].points.clone();
        assert_eq!(next[0].submitted_at, old[0].submitted_at);
        let mut unknown = point("x");
        unknown.id = Some("unknown".into());
        assert!(store
            .upsert("p".into(), "a".into(), "main".into(), vec![unknown])
            .is_err());
    }

    #[test]
    fn v2_requires_ids_and_dismiss_migrates_v1_on_write() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        fs::write(&path, r#"{"version":2,"entries":[{"folder_name":"p","session_id":"a","branch":"main","points":[{"text":"x"}],"updated_at":"2025-01-01T00:00:00Z"}]}"#).unwrap();
        assert!(StatusStore::new(path.clone()).read().is_err());
        fs::write(&path, r#"{"version":1,"entries":[{"folder_name":"p","session_id":"a","branch":"main","points":[{"text":"x"}],"updated_at":"2025-01-01T00:00:00Z"}]}"#).unwrap();
        let store = StatusStore::new(path.clone());
        let entry = store.read().unwrap().entries[0].clone();
        assert!(store
            .dismiss(
                "p",
                Some("a"),
                "main",
                &entry.updated_at,
                entry.points[0].id.as_deref().unwrap(),
                None
            )
            .unwrap());
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&fs::read(path).unwrap()).unwrap()
                ["version"],
            2
        );
    }
    #[test]
    fn prune_removes_old_points_and_keeps_recent_documents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("status.json");
        let now = chrono::Utc::now();
        let document = Document {
            version: 2,
            entries: vec![Entry {
                repository: None,
                folder_name: "p".into(),
                session_id: Some("a".into()),
                branch: "main".into(),
                points: vec![
                    Point {
                        text: "old".into(),
                        done: false,
                        pinned: false,
                        link: None,
                        submitted_at: Some((now - chrono::Duration::hours(2)).to_rfc3339()),
                        id: Some("old".into()),
                    },
                    Point {
                        text: "new".into(),
                        done: false,
                        pinned: false,
                        link: None,
                        submitted_at: Some(now.to_rfc3339()),
                        id: Some("new".into()),
                    },
                ],
                updated_at: now.to_rfc3339(),
            }],
        };
        fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap();
        assert!(StatusStore::new(path.clone())
            .prune(Duration::from_secs(60))
            .unwrap());
        assert_eq!(
            StatusStore::new(path).read().unwrap().entries[0].points[0]
                .id
                .as_deref(),
            Some("new")
        );
    }
    #[test]
    fn malformed_and_future_documents_are_not_rewritten() {
        let dir = tempfile::tempdir().unwrap();
        for (name, contents) in [
            ("bad", "broken"),
            ("future", r#"{"version":99,"entries":[]}"#),
        ] {
            let path = dir.path().join(name);
            fs::write(&path, contents).unwrap();
            let before = fs::read(&path).unwrap();
            assert!(StatusStore::new(path.clone())
                .upsert("p".into(), "a".into(), "main".into(), vec![point("x")])
                .is_err());
            assert_eq!(fs::read(path).unwrap(), before);
        }
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    fn point(text: &str) -> Point {
        Point {
            text: text.into(),
            done: false,
            pinned: false,
            link: None,
            submitted_at: None,
            id: None,
        }
    }
    fn setup() -> (tempfile::TempDir, StatusStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = StatusStore::new(dir.path().join("status.json"));
        store
            .upsert(
                "project".into(),
                "session".into(),
                "main".into(),
                vec![point("keep"), point("work")],
            )
            .unwrap();
        (dir, store)
    }
    fn pin_first(store: &StatusStore) -> Entry {
        let entry = store.read().unwrap().entries.remove(0);
        store
            .set_point_pinned(
                "project",
                Some("session"),
                "main",
                &entry.updated_at,
                entry.points[0].id.as_deref().unwrap(),
                true,
            )
            .unwrap();
        entry
    }
    #[test]
    fn private_pins_survive_legacy_replacement_and_raw_wire_has_no_pin_fields() {
        let (_dir, store) = setup();
        let saved = pin_first(&store);
        let raw: serde_json::Value =
            serde_json::from_slice(&fs::read(&store.path).unwrap()).unwrap();
        assert_eq!(raw["entries"][0]["points"].as_array().unwrap().len(), 1);
        assert!(raw["entries"][0]["points"][0].get("pinned").is_none());
        let mut legacy = raw;
        legacy["entries"][0]["points"][0]["text"] = serde_json::json!("legacy update");
        fs::write(&store.path, serde_json::to_vec(&legacy).unwrap()).unwrap();
        let doc = store.read().unwrap();
        assert!(doc.entries[0]
            .points
            .iter()
            .any(|p| p.pinned && p.id == saved.points[0].id));
        assert!(doc.entries[0]
            .points
            .iter()
            .any(|p| p.text == "legacy update"));
        store
            .upsert(
                "project".into(),
                "session".into(),
                "feature".into(),
                vec![point("new branch")],
            )
            .unwrap();
        assert!(store
            .read()
            .unwrap()
            .entries
            .iter()
            .any(|e| e.session_id.is_none() && e.branch == "main" && e.points[0].pinned));
    }
    #[test]
    fn reading_inline_pin_format_migrates_once_and_preserves_original_backup() {
        let (_dir, store) = setup();
        let mut doc = store.read().unwrap();
        doc.entries[0].points[0].pinned = true;
        let original = serde_json::to_vec(&doc).unwrap();
        fs::write(&store.path, &original).unwrap();
        assert!(store.read().unwrap().entries[0]
            .points
            .iter()
            .any(|p| p.pinned));
        let raw = fs::read_to_string(&store.path).unwrap();
        assert!(!raw.contains("\"pinned\""));
        let backups = fs::read_dir(store.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|x| x == "backup"))
            .collect::<Vec<_>>();
        assert_eq!(backups.len(), 1);
        assert_eq!(fs::read(backups[0].path()).unwrap(), original);
        store.read().unwrap();
        assert_eq!(fs::read_dir(store.parent().unwrap()).unwrap().count(), 4);
    }
    #[test]
    fn interrupted_pin_writes_recover_and_never_overwrite_newer_legacy_data() {
        for mode in ["before", "after", "legacy"] {
            let (_dir, store) = setup();
            pin_first(&store);
            let before = fs::read(&store.path).unwrap();
            let mut after: serde_json::Value = serde_json::from_slice(&before).unwrap();
            after["entries"][0]["points"][0]["text"] = serde_json::json!("pending write");
            let after = serde_json::to_vec(&after).unwrap();
            let mut private = store.read_private().unwrap();
            private.pending = Some(PendingWrite {
                before_raw: Some(String::from_utf8(before).unwrap()),
                after_raw: String::from_utf8(after.clone()).unwrap(),
                after_pins: vec![],
            });
            store.save_private(&private).unwrap();
            if mode == "after" {
                fs::write(&store.path, &after).unwrap();
            }
            if mode == "legacy" {
                let mut newer: serde_json::Value = serde_json::from_slice(&after).unwrap();
                newer["entries"][0]["points"][0]["text"] = serde_json::json!("newer legacy");
                fs::write(&store.path, serde_json::to_vec(&newer).unwrap()).unwrap();
            }
            let doc = store.read().unwrap();
            if mode == "legacy" {
                assert!(doc.entries[0]
                    .points
                    .iter()
                    .any(|p| p.text == "newer legacy"));
                assert!(doc.entries[0].points.iter().any(|p| p.pinned));
            } else {
                assert_eq!(doc.entries[0].points[0].text, "pending write");
                assert!(!doc.entries[0].points.iter().any(|p| p.pinned));
            }
            assert!(store.read_private().unwrap().pending.is_none());
        }
    }
    #[test]
    fn lock_wait_is_bounded_then_succeeds_after_release() {
        let (_dir, store) = setup();
        let lock = store.lock().unwrap();
        assert!(matches!(
            store.lock_for(Duration::from_millis(20)),
            Err(StatusError::LockTimeout(_))
        ));
        drop(lock);
        assert!(store.lock_for(Duration::from_millis(20)).is_ok());
    }
    #[test]
    fn archive_retention_is_idempotent_and_restore_is_pinned() {
        let (_dir, store) = setup();
        assert_eq!(
            store
                .preview_retention(&[domain::RetentionRule::CountPerProject(1)])
                .unwrap(),
            1
        );
        store
            .apply_retention(&[domain::RetentionRule::CountPerProject(1)])
            .unwrap();
        store
            .apply_retention(&[domain::RetentionRule::CountPerProject(1)])
            .unwrap();
        let removed = store.list_removed().unwrap();
        assert_eq!(removed.len(), 1);
        store.restore_removed(&removed[0].id).unwrap();
        assert!(store.list_removed().unwrap().is_empty());
        store
            .apply_retention(&[
                domain::RetentionRule::Age(Duration::ZERO),
                domain::RetentionRule::CountPerProject(0),
            ])
            .unwrap();
        assert!(store
            .read()
            .unwrap()
            .entries
            .iter()
            .flat_map(|e| &e.points)
            .any(|p| p.pinned));
        assert!(store.restore_removed(&removed[0].id).is_err());
    }
    #[test]
    fn corrupt_private_data_prevents_pruning_and_keeps_status_bytes() {
        let (_dir, store) = setup();
        let before = fs::read(&store.path).unwrap();
        fs::write(store.private_path(), "broken").unwrap();
        assert!(store
            .apply_retention(&[domain::RetentionRule::CountPerProject(0)])
            .is_err());
        assert_eq!(fs::read(&store.path).unwrap(), before);
        assert_eq!(fs::read_to_string(store.private_path()).unwrap(), "broken");
    }
    #[test]
    fn archive_restore_preserves_a_newer_session_branch() {
        let (_dir, store) = setup();
        store
            .apply_retention(&[domain::RetentionRule::CountPerProject(1)])
            .unwrap();
        let removed = store.list_removed().unwrap();
        store
            .upsert(
                "project".into(),
                "session".into(),
                "feature".into(),
                vec![point("new work")],
            )
            .unwrap();
        store.restore_removed(&removed[0].id).unwrap();
        let doc = store.read().unwrap();
        assert!(doc
            .entries
            .iter()
            .any(|entry| entry.branch == "feature" && entry.points[0].text == "new work"));
        assert!(doc.entries.iter().any(|entry| entry.branch == "main"
            && entry.session_id.is_none()
            && entry.points[0].pinned));
    }

    #[test]
    fn preserved_legacy_executable_can_update_while_private_pins_survive() {
        let executable = std::path::Path::new("/tmp/sideglance-legacy-remediation-mcp");
        if !executable.is_file() {
            return;
        }
        let home = tempfile::tempdir().unwrap();
        let store = StatusStore::new(home.path().join(".local/share/sideglance/status.json"));
        store
            .upsert(
                "project".into(),
                "session".into(),
                "main".into(),
                vec![point("keep"), point("work")],
            )
            .unwrap();
        pin_first(&store);
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            use tokio::io::{AsyncBufReadExt,AsyncWriteExt,BufReader};
            let mut child=tokio::process::Command::new(executable).arg("--mcp").env("HOME",home.path()).stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null()).kill_on_drop(true).spawn().unwrap();
            let mut input=child.stdin.take().unwrap();let mut output=BufReader::new(child.stdout.take().unwrap()).lines();
            let result=tokio::time::timeout(Duration::from_secs(4),async {
                let initialize=serde_json::json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"legacy-test","version":"1"}}});
                input.write_all(format!("{initialize}\n").as_bytes()).await.unwrap();
                let init:serde_json::Value=serde_json::from_str(&output.next_line().await.unwrap().unwrap()).unwrap();assert!(init.get("error").is_none());
                let ready=serde_json::json!({"jsonrpc":"2.0","method":"notifications/initialized"});
                let write=serde_json::json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"write_status","arguments":{"folder_name":"project","session_id":"session","branch":"main","points":[{"text":"legacy work"}]}}});
                input.write_all(format!("{ready}\n{write}\n").as_bytes()).await.unwrap();
                let reply:serde_json::Value=serde_json::from_str(&output.next_line().await.unwrap().unwrap()).unwrap();
                assert!(reply.get("error").is_none(),"{reply}");assert_ne!(reply.pointer("/result/isError"),Some(&serde_json::json!(true)));
            }).await;
            drop(input);drop(output);let _=child.kill().await;let _=child.wait().await;result.unwrap();
        });
        let doc = store.read().unwrap();
        assert!(doc.entries[0]
            .points
            .iter()
            .any(|p| p.pinned && p.text == "keep"));
        assert!(doc.entries[0]
            .points
            .iter()
            .any(|p| p.text == "legacy work"));
    }
    #[test]
    fn interrupted_new_pin_survives_an_intervening_legacy_write() {
        let (_dir, store) = setup();
        let original = fs::read(&store.path).unwrap();
        let mut doc = store.read().unwrap();
        let mut saved = doc.entries[0].clone();
        saved.points = vec![saved.points[0].clone()];
        saved.points[0].pinned = true;
        doc.entries[0].points.remove(0);
        let mut after = serde_json::to_value(doc).unwrap();
        for point in after["entries"][0]["points"].as_array_mut().unwrap() {
            point.as_object_mut().unwrap().remove("pinned");
        }
        let after = serde_json::to_string(&after).unwrap();
        let mut private = store.read_private().unwrap();
        private.pending = Some(PendingWrite {
            before_raw: Some(String::from_utf8(original).unwrap()),
            after_raw: after.clone(),
            after_pins: vec![saved],
        });
        store.save_private(&private).unwrap();
        let mut legacy: serde_json::Value = serde_json::from_str(&after).unwrap();
        legacy["entries"][0]["points"][0]["text"] = serde_json::json!("newer legacy report");
        let newer = serde_json::to_vec(&legacy).unwrap();
        fs::write(&store.path, &newer).unwrap();
        let doc = store.read().unwrap();
        assert!(doc.entries[0]
            .points
            .iter()
            .any(|point| point.pinned && point.text == "keep"));
        assert!(doc.entries[0]
            .points
            .iter()
            .any(|point| point.text == "newer legacy report"));
        assert_eq!(fs::read(&store.path).unwrap(), newer);
        assert!(store.read_private().unwrap().pending.is_none());
    }
    #[test]
    fn legacy_age_deletion_records_the_correct_reason() {
        let (_dir, store) = setup();
        let mut doc = store.read().unwrap();
        doc.entries[0].updated_at = (Utc::now() - chrono::Duration::hours(2)).to_rfc3339();
        for point in &mut doc.entries[0].points {
            point.submitted_at = None;
        }
        fs::write(&store.path, serde_json::to_vec(&doc).unwrap()).unwrap();
        store.prune(Duration::from_secs(60)).unwrap();
        assert!(store
            .list_removed()
            .unwrap()
            .iter()
            .all(|item| item.reason == "Time-based deletion"));
    }
    #[test]
    fn pending_pin_merge_normalizes_session_identity_across_branches() {
        let (_dir, store) = setup();
        pin_first(&store);
        let mut raw: serde_json::Value =
            serde_json::from_slice(&fs::read(&store.path).unwrap()).unwrap();
        raw["entries"][0]["branch"] = serde_json::json!("feature");
        fs::write(&store.path, serde_json::to_vec(&raw).unwrap()).unwrap();
        let before = fs::read_to_string(&store.path).unwrap();
        let doc = store.read().unwrap();
        let mut after_pins = doc.entries.clone();
        for entry in &mut after_pins {
            for point in &mut entry.points {
                point.pinned = true;
            }
        }
        let mut private = store.read_private().unwrap();
        private.pending = Some(PendingWrite {
            before_raw: Some(before),
            after_raw: r#"{"version":2,"entries":[]}"#.into(),
            after_pins,
        });
        store.save_private(&private).unwrap();
        raw["entries"][0]["points"][0]["id"] = serde_json::json!("later-point");
        raw["entries"][0]["points"][0]["text"] = serde_json::json!("later report");
        fs::write(&store.path, serde_json::to_vec(&raw).unwrap()).unwrap();
        let doc = store.read().unwrap();
        assert_eq!(doc.entries.iter().flat_map(|e| &e.points).count(), 3);
        assert!(doc
            .entries
            .iter()
            .any(|e| e.branch == "main" && e.session_id.is_none() && e.points[0].pinned));
        assert!(doc
            .entries
            .iter()
            .any(|e| e.branch == "feature" && e.points.iter().any(|p| p.pinned)));
        store.read().unwrap(); // the normalized private state must stay readable.
    }
    #[test]
    fn automatic_history_trims_oldest_records_by_age_and_byte_cap() {
        let (_dir, store) = setup();
        let entry = store.read().unwrap().entries.remove(0);
        let mut archive = Vec::new();
        for index in 0..8 {
            let mut saved = entry.clone();
            saved.points.truncate(1);
            saved.points[0].text = "x".repeat(850_000);
            archive.push(RemovedPoint {
                id: format!("item-{index}"),
                entry: saved,
                point_index: 0,
                removed_at: (Utc::now()
                    - chrono::Duration::days(if index == 0 { 40 } else { 8 - index }))
                .to_rfc3339(),
                reason: "Time-based deletion".into(),
            });
        }
        trim_archive(&mut archive).unwrap();
        assert!(!archive.iter().any(|item| item.id == "item-0"));
        assert!(!archive.iter().any(|item| item.id == "item-1"));
        assert!(archive.iter().any(|item| item.id == "item-7"));
        assert!(serde_json::to_vec(&archive).unwrap().len() <= MAX_ARCHIVE_BYTES);
    }
}
