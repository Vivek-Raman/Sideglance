use crate::{
    preferences::SettingsOwner,
    status::retention::clear_expired_points,
    status::snapshot::{
        publish_retention_error, publish_watcher_error, retention_recovered, update,
        watcher_recovered, ViewerState,
    },
};
use notify::{RecursiveMode, Watcher};
use std::{
    sync::{mpsc, Arc},
    thread,
    time::{Duration, Instant},
};
use tauri::AppHandle;

const QUIET_PERIOD: Duration = Duration::from_millis(120);
const MAX_BURST_WAIT: Duration = Duration::from_millis(500);
const RECONCILE_PERIOD: Duration = Duration::from_secs(30);
const INITIAL_RETRY: Duration = Duration::from_millis(250);

// The maximum burst age prevents a busy writer from postponing reads forever.
struct RefreshSchedule {
    reconcile_at: Instant,
    dirty_since: Option<Instant>,
    quiet_at: Option<Instant>,
}
impl RefreshSchedule {
    fn new(now: Instant) -> Self {
        Self {
            reconcile_at: now + RECONCILE_PERIOD,
            dirty_since: None,
            quiet_at: None,
        }
    }
    fn changed(&mut self, now: Instant) {
        self.dirty_since.get_or_insert(now);
        self.quiet_at = Some(now + QUIET_PERIOD);
    }
    fn deadline(&self) -> Instant {
        let dirty = self
            .dirty_since
            .map(|since| self.quiet_at.unwrap().min(since + MAX_BURST_WAIT));
        dirty.map_or(self.reconcile_at, |at| at.min(self.reconcile_at))
    }
    fn due(&self, now: Instant) -> bool {
        now >= self.deadline()
    }
    fn refreshed(&mut self, now: Instant) {
        self.dirty_since = None;
        self.quiet_at = None;
        self.reconcile_at = now + RECONCILE_PERIOD;
    }
}

pub fn start(app: AppHandle, state: Arc<ViewerState>, owner: Arc<SettingsOwner>) {
    thread::spawn(move || {
        let (tx, rx) = mpsc::channel();
        let mut watcher = None;
        let parent = state
            .path
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .to_path_buf();
        let mut schedule = RefreshSchedule::new(Instant::now());
        let mut next_watch_attempt = Instant::now();
        let mut retry_delay = INITIAL_RETRY;
        loop {
            let now = Instant::now();
            if watcher.is_none() && now >= next_watch_attempt {
                let attempt =
                    notify::recommended_watcher(tx.clone()).and_then(|mut new_watcher| {
                        new_watcher.watch(&parent, RecursiveMode::NonRecursive)?;
                        Ok(new_watcher)
                    });
                match attempt {
                    Ok(new_watcher) => {
                        watcher = Some(new_watcher);
                        retry_delay = INITIAL_RETRY;
                        watcher_recovered(&app, &state);
                        // Reconcile changes made while registration was unavailable.
                        schedule.changed(now);
                    }
                    Err(error) => {
                        publish_watcher_error(
                            &app,
                            &state,
                            format!("Unable to watch the status directory: {error}"),
                        );
                        next_watch_attempt = now + retry_delay;
                        retry_delay = (retry_delay * 2).min(RECONCILE_PERIOD);
                    }
                }
            }
            let deadline = if watcher.is_none() {
                schedule.deadline().min(next_watch_attempt)
            } else {
                schedule.deadline()
            };
            match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
                Ok(Ok(event)) => {
                    if !matches!(event.kind, notify::EventKind::Access(_))
                        && (event.need_rescan()
                            || event
                                .paths
                                .iter()
                                .any(|path| path == &state.path || path == &parent))
                    {
                        schedule.changed(Instant::now());
                    }
                }
                Ok(Err(error)) => {
                    publish_watcher_error(&app, &state, format!("Status watcher error: {error}"));
                    watcher = None;
                    next_watch_attempt = Instant::now() + retry_delay;
                    retry_delay = (retry_delay * 2).min(RECONCILE_PERIOD);
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    // tx is retained by this thread; disconnect is terminal rather than a hot retry loop.
                    publish_watcher_error(&app, &state, "Status watcher stopped");
                    return;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
            }
            // This runs after every event, including unrelated directory activity.
            if schedule.due(Instant::now()) {
                if let Err(error) = clear_expired_points(&state, &owner) {
                    publish_retention_error(
                        &app,
                        &state,
                        format!("Unable to prune status points: {error}"),
                    );
                } else {
                    retention_recovered(&app, &state);
                }
                update(&app, &state);
                schedule.refreshed(Instant::now());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn burst_waits_for_quiet_but_has_a_maximum_age() {
        let now = Instant::now();
        let mut schedule = RefreshSchedule::new(now);
        schedule.changed(now);
        schedule.changed(now + Duration::from_millis(100));
        assert!(!schedule.due(now + Duration::from_millis(219)));
        assert!(schedule.due(now + Duration::from_millis(220)));
        for ms in [200, 300, 400, 490] {
            schedule.changed(now + Duration::from_millis(ms));
        }
        assert!(!schedule.due(now + Duration::from_millis(499)));
        assert!(schedule.due(now + MAX_BURST_WAIT));
    }

    #[test]
    fn unrelated_activity_cannot_postpone_reconciliation_and_idle_wait_is_long() {
        let now = Instant::now();
        let mut schedule = RefreshSchedule::new(now);
        assert_eq!(schedule.deadline(), now + RECONCILE_PERIOD);
        // Unrelated events do not call changed. Reconciliation is checked after each event.
        for second in 1..30 {
            assert!(!schedule.due(now + Duration::from_secs(second)));
        }
        assert!(schedule.due(now + RECONCILE_PERIOD));
        schedule.refreshed(now + RECONCILE_PERIOD);
        assert_eq!(schedule.deadline(), now + RECONCILE_PERIOD * 2);
    }

    #[test]
    fn a_nearby_periodic_deadline_wins_over_continuous_changes() {
        let now = Instant::now();
        let mut schedule = RefreshSchedule::new(now);
        schedule.changed(now + RECONCILE_PERIOD - Duration::from_millis(10));
        assert_eq!(schedule.deadline(), now + RECONCILE_PERIOD);
    }
}
