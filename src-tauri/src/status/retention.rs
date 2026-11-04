use crate::{
    preferences::SettingsOwner,
    status::{domain::RetentionRule, snapshot::ViewerState},
};
use std::time::Duration;

pub fn clear_expired_points(state: &ViewerState, owner: &SettingsOwner) -> Result<bool, String> {
    let settings = owner.snapshot();
    let mut rules = Vec::new();
    if let Some(minutes) = settings.auto_clear_after_minutes {
        rules.push(RetentionRule::Age(Duration::from_secs(
            minutes.saturating_mul(60),
        )));
    }
    if let Some(count) = settings.max_status_items_per_project {
        rules.push(RetentionRule::CountPerProject(count));
    }
    if rules.is_empty() {
        return Ok(false);
    }
    state
        .store
        .apply_retention(&rules)
        .map_err(|error| error.to_string())
}
