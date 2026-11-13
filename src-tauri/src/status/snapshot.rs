use crate::status::store::{StatusStore, MAX_STATUS_BYTES};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tauri::{AppHandle, Emitter, Manager};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Health {
    Waiting,
    Current,
    Stale,
    Error,
}
#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct Snapshot {
    pub path: Option<String>,
    pub document: crate::status::domain::Document,
    pub revision: u64,
    pub health: Health,
    pub error: Option<String>,
    pub modified_at: Option<String>,
    pub last_successful_read_at: Option<String>,
    pub recovery_warning: Option<String>,
}

pub struct ViewerState {
    pub path: PathBuf,
    pub store: StatusStore,
    pub snapshot: Mutex<Snapshot>,
    coordinator: RefreshCoordinator,
    watcher_error: Mutex<Option<String>>,
    retention_error: Mutex<Option<String>>,
}
impl ViewerState {
    pub fn new(path: PathBuf) -> Arc<Self> {
        Arc::new(Self {
            store: StatusStore::new(path.clone()),
            path,
            snapshot: Mutex::new(Snapshot {
                path: None,
                document: Default::default(),
                revision: 0,
                health: Health::Waiting,
                error: None,
                modified_at: None,
                last_successful_read_at: None,
                recovery_warning: None,
            }),
            coordinator: RefreshCoordinator::new(),
            watcher_error: Mutex::new(None),
            retention_error: Mutex::new(None),
        })
    }
}

struct RefreshCoordinator {
    serial: Mutex<()>,
}
impl RefreshCoordinator {
    fn new() -> Self {
        Self {
            serial: Mutex::new(()),
        }
    }
    fn refresh<R, P>(&self, read: R, publish: P)
    where
        R: FnOnce() -> Snapshot,
        P: FnOnce(Snapshot),
    {
        let _serial = self.serial.lock().unwrap();
        let next = read();
        publish(next);
    }
}

fn modified_at(path: &std::path::Path) -> Option<String> {
    std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|time| chrono::DateTime::<chrono::Local>::from(time).to_rfc3339())
}
fn snapshot_for(state: &ViewerState, previous: &Snapshot) -> Snapshot {
    let modified_at = modified_at(&state.path);
    let warning = state
        .watcher_error
        .lock()
        .unwrap()
        .clone()
        .or_else(|| state.retention_error.lock().unwrap().clone());
    let stale = |health, error| Snapshot {
        path: Some(state.path.display().to_string()),
        document: previous.document.clone(),
        revision: previous.revision.saturating_add(1),
        health,
        error,
        modified_at: modified_at.clone(),
        last_successful_read_at: previous.last_successful_read_at.clone(),
        recovery_warning: previous.recovery_warning.clone(),
    };
    match std::fs::metadata(&state.path) {
        Ok(meta) if !meta.is_file() => stale(
            Health::Error,
            Some("The selected path is not a regular file".into()),
        ),
        Ok(meta) if meta.len() > MAX_STATUS_BYTES as u64 => stale(
            Health::Error,
            Some("The file is larger than the 1 MiB limit".into()),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if previous.last_successful_read_at.is_some() {
                stale(
                    Health::Stale,
                    Some(
                        "Status file is missing. Showing the last successfully read updates."
                            .into(),
                    ),
                )
            } else if warning.is_some() {
                stale(Health::Error, warning.clone())
            } else {
                stale(Health::Waiting, None)
            }
        }
        Err(error) => stale(Health::Error, Some(format!("Unable to read file: {error}"))),
        Ok(_) => match state.store.read() {
            Ok(document) => Snapshot {
                path: Some(state.path.display().to_string()),
                document,
                revision: previous.revision.saturating_add(1),
                health: if warning.is_some() {
                    Health::Error
                } else {
                    Health::Current
                },
                error: warning,
                modified_at,
                last_successful_read_at: Some(chrono::Utc::now().to_rfc3339()),
                recovery_warning: previous.recovery_warning.clone(),
            },
            Err(error) => stale(Health::Stale, Some(error.to_string())),
        },
    }
}

pub fn update(app: &AppHandle, state: &ViewerState) {
    state.coordinator.refresh(
        || {
            let previous = state.snapshot.lock().unwrap().clone();
            let mut next = snapshot_for(state, &previous);
            next.recovery_warning = app
                .try_state::<Arc<crate::preferences::SettingsOwner>>()
                .and_then(|owner| owner.recovery_warning());
            next
        },
        |snapshot| {
            if let Some(snapshot) = cache_snapshot(&state.snapshot, snapshot) {
                let _ = app.emit("source-update", snapshot);
            }
        },
    );
}
// The refresh coordinator serializes this cache commit with publication.
fn cache_snapshot(slot: &Mutex<Snapshot>, mut snapshot: Snapshot) -> Option<Snapshot> {
    let mut current = slot.lock().unwrap();
    let changed = meaningful_change(&current, &snapshot);
    snapshot.revision = if changed {
        current.revision.saturating_add(1)
    } else {
        current.revision
    };
    *current = snapshot.clone();
    changed.then_some(snapshot)
}

// Read timestamps and filesystem metadata are cached without creating layout revisions.
fn meaningful_change(previous: &Snapshot, next: &Snapshot) -> bool {
    previous.path != next.path
        || previous.document != next.document
        || previous.health != next.health
        || previous.error != next.error
        || previous.recovery_warning != next.recovery_warning
}

fn set_warning(slot: &Mutex<Option<String>>, next: Option<String>) -> bool {
    let mut current = slot.lock().unwrap();
    if *current == next {
        return false;
    }
    *current = next;
    true
}

pub fn publish_watcher_error(app: &AppHandle, state: &ViewerState, message: impl Into<String>) {
    if set_warning(&state.watcher_error, Some(message.into())) {
        update(app, state);
    }
}
pub fn watcher_recovered(app: &AppHandle, state: &ViewerState) {
    if set_warning(&state.watcher_error, None) {
        update(app, state);
    }
}
pub fn publish_retention_error(app: &AppHandle, state: &ViewerState, message: impl Into<String>) {
    if set_warning(&state.retention_error, Some(message.into())) {
        update(app, state);
    }
}
pub fn retention_recovered(app: &AppHandle, state: &ViewerState) {
    if set_warning(&state.retention_error, None) {
        update(app, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status::domain::Point;
    use std::sync::{Arc, Barrier};

    #[test]
    fn concurrent_refreshes_publish_in_revision_order() {
        let coordinator = Arc::new(RefreshCoordinator::new());
        let snapshot = Arc::new(Mutex::new(Snapshot {
            path: None,
            document: Default::default(),
            revision: 0,
            health: Health::Waiting,
            error: None,
            modified_at: None,
            last_successful_read_at: None,
            recovery_warning: None,
        }));
        let barrier = Arc::new(Barrier::new(3));
        let published = Arc::new(Mutex::new(Vec::new()));
        std::thread::scope(|scope| {
            for _ in 0..2 {
                let coordinator = coordinator.clone();
                let barrier = barrier.clone();
                let published = published.clone();
                let snapshot = snapshot.clone();
                scope.spawn(move || {
                    barrier.wait();
                    coordinator.refresh(
                        || {
                            let mut next = snapshot.lock().unwrap().clone();
                            next.revision += 1;
                            next
                        },
                        |next| {
                            *snapshot.lock().unwrap() = next.clone();
                            published.lock().unwrap().push(next.revision);
                        },
                    );
                });
            }
            barrier.wait();
        });
        assert_eq!(*published.lock().unwrap(), vec![1, 2]);
    }

    #[test]
    fn successful_reads_replace_the_document_during_a_watcher_outage() {
        let directory = tempfile::tempdir().unwrap();
        let state = ViewerState::new(directory.path().join("status.json"));
        state
            .store
            .upsert(
                "project".into(),
                "session".into(),
                "main".into(),
                vec![Point {
                    text: "first".into(),
                    done: false,
                    pinned: false,
                    link: None,
                    submitted_at: None,
                    id: None,
                }],
            )
            .unwrap();
        let first = snapshot_for(&state, &state.snapshot.lock().unwrap());
        *state.snapshot.lock().unwrap() = first.clone();
        *state.watcher_error.lock().unwrap() = Some("watch unavailable".into());
        state
            .store
            .upsert(
                "project".into(),
                "session".into(),
                "main".into(),
                vec![Point {
                    text: "second".into(),
                    done: false,
                    pinned: false,
                    link: None,
                    submitted_at: None,
                    id: first.document.entries[0].points[0].id.clone(),
                }],
            )
            .unwrap();
        let second = snapshot_for(&state, &first);
        assert_eq!(second.document.entries[0].points[0].text, "second");
        assert_eq!(second.error.as_deref(), Some("watch unavailable"));
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;

    fn sample() -> (tempfile::TempDir, Arc<ViewerState>) {
        let directory = tempfile::tempdir().unwrap();
        let state = ViewerState::new(directory.path().join("status.json"));
        std::fs::write(&state.path, r#"{"version":1,"entries":[{"folder_name":"project","session_id":"s","branch":"main","points":[{"text":"working"}],"updated_at":"2026-01-01T00:00:00Z"}]}"#).unwrap();
        (directory, state)
    }

    #[test]
    fn unchanged_reads_cache_fresh_read_times_without_meaningful_change() {
        let (_directory, state) = sample();
        let first = snapshot_for(&state, &state.snapshot.lock().unwrap());
        assert!(first.last_successful_read_at.is_some());
        let second = snapshot_for(&state, &first);
        assert!(!meaningful_change(&first, &second));
        let mut timestamp_only = second.clone();
        timestamp_only.modified_at = Some("different filesystem timestamp".into());
        timestamp_only.last_successful_read_at = Some("later successful read".into());
        assert!(!meaningful_change(&first, &timestamp_only));
        let mut warning = second.clone();
        warning.error = Some("watcher failed".into());
        warning.health = Health::Error;
        assert!(meaningful_change(&first, &warning));
        warning = second.clone();
        warning.recovery_warning = Some("Settings recovered".into());
        assert!(meaningful_change(&first, &warning));
    }

    #[test]
    fn disappearing_file_retains_last_good_document_with_visible_stale_warning() {
        let (_directory, state) = sample();
        let first = snapshot_for(&state, &state.snapshot.lock().unwrap());
        std::fs::remove_file(&state.path).unwrap();
        let missing = snapshot_for(&state, &first);
        assert_eq!(missing.health, Health::Stale);
        assert!(missing.error.as_deref().unwrap().contains("missing"));
        assert_eq!(missing.document, first.document);
        assert_eq!(
            missing.last_successful_read_at,
            first.last_successful_read_at
        );
        let startup = snapshot_for(&state, &state.snapshot.lock().unwrap());
        assert_eq!(startup.health, Health::Waiting);
        assert!(startup.last_successful_read_at.is_none());
    }

    #[test]
    fn cached_reconciliation_only_advances_revisions_for_meaningful_changes() {
        let (_directory, state) = sample();
        let initial = state.snapshot.lock().unwrap().clone();
        let first = snapshot_for(&state, &initial);
        assert_eq!(cache_snapshot(&state.snapshot, first).unwrap().revision, 1);
        let previous = state.snapshot.lock().unwrap().clone();
        let same = snapshot_for(&state, &previous);
        assert!(cache_snapshot(&state.snapshot, same).is_none());
        assert_eq!(state.snapshot.lock().unwrap().revision, 1);
        assert!(set_warning(
            &state.watcher_error,
            Some("watch unavailable".into())
        ));
        let warning = snapshot_for(&state, &previous);
        assert_eq!(
            cache_snapshot(&state.snapshot, warning).unwrap().revision,
            2
        );
        set_warning(&state.watcher_error, None);
        let previous = state.snapshot.lock().unwrap().clone();
        let recovered = snapshot_for(&state, &previous);
        assert_eq!(
            cache_snapshot(&state.snapshot, recovered).unwrap().revision,
            3
        );
        let previous = state.snapshot.lock().unwrap().clone();
        assert!(cache_snapshot(&state.snapshot, snapshot_for(&state, &previous)).is_none());
    }

    #[test]
    fn parse_failures_keep_successful_read_timestamp_and_warning_changes_are_transitions() {
        let (_directory, state) = sample();
        let first = snapshot_for(&state, &state.snapshot.lock().unwrap());
        std::fs::write(&state.path, "invalid").unwrap();
        let failed = snapshot_for(&state, &first);
        assert_eq!(failed.health, Health::Stale);
        assert_eq!(failed.document, first.document);
        assert_eq!(
            failed.last_successful_read_at,
            first.last_successful_read_at
        );
        assert!(set_warning(&state.watcher_error, Some("error".into())));
        assert!(!set_warning(&state.watcher_error, Some("error".into())));
        assert!(set_warning(&state.watcher_error, None));
        assert!(!set_warning(&state.watcher_error, None));
    }
}
