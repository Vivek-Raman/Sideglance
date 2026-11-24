mod project;

use crate::{
    paths,
    status::domain::Point,
    status::store::{StatusError, StatusStore},
};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    tool, tool_handler, tool_router, ErrorData as McpError, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

// Keep storage-only fields out of the agent protocol and its tool schema.
#[derive(Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AgentPoint {
    /// Brief plain text status, without Markdown or commit hashes.
    text: String,
    /// Set true when this work is complete.
    #[serde(default)]
    done: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Optional HTTP(S) link; use the canonical PR URL for PR work.
    link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    /// Preserve returned IDs for existing points; omit for new points.
    id: Option<String>,
}

impl From<AgentPoint> for Point {
    fn from(point: AgentPoint) -> Self {
        Self {
            text: point.text,
            done: point.done,
            pinned: false,
            link: point.link,
            submitted_at: None,
            id: point.id,
        }
    }
}

impl From<Point> for AgentPoint {
    fn from(point: Point) -> Self {
        Self {
            text: point.text,
            done: point.done,
            link: point.link,
            id: point.id,
        }
    }
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct WriteStatusInput {
    /// Absolute working directory. The server derives project, branch and repository using local Git.
    working_directory: String,
    /// Your stable session ID from the harness.
    session_id: String,
    /// Complete revised point list; omitted points are removed.
    points: Vec<AgentPoint>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadStatusInput {
    /// Absolute working directory in the project; subdirectories resolve to the Git checkout root.
    working_directory: String,
    /// Your stable session ID from the harness.
    session_id: String,
}

struct StatusServer {
    tool_router: ToolRouter<Self>,
}

fn map_status_error(error: StatusError) -> McpError {
    match error {
        StatusError::InvalidInput(message) => McpError::invalid_params(message, None),
        StatusError::Conflict(message) => McpError::invalid_params(message, None),
        StatusError::InvalidDocument(message)
        | StatusError::Io(message)
        | StatusError::LockTimeout(message) => McpError::internal_error(message, None),
    }
}

impl StatusServer {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[tool_router(router = tool_router)]
impl StatusServer {
    #[tool(
        description = "Read your points when starting or recovering context. Returns points with stable IDs."
    )]
    async fn read_status(
        &self,
        Parameters(input): Parameters<ReadStatusInput>,
    ) -> Result<String, McpError> {
        tokio::task::spawn_blocking(move || {
            let project = project::resolve(&input.working_directory)
                .map_err(|message| McpError::invalid_params(message, None))?;
            let path = paths::status_path()
                .map_err(|error| McpError::internal_error(error.to_string(), None))?;
            let entries = StatusStore::new(path)
                .read_project(&project.folder_name, &input.session_id)
                .map_err(map_status_error)?;
            let points: Vec<AgentPoint> = entries
                .into_iter()
                .flat_map(|entry| entry.points)
                .map(Into::into)
                .collect();
            serde_json::to_string(&points)
                .map_err(|error| McpError::internal_error(error.to_string(), None))
        })
        .await
        .map_err(|error| McpError::internal_error(format!("Status worker failed: {error}"), None))?
    }

    #[tool(
        description = "Replace your points; omitted points are removed. Returns saved points with IDs for the next update."
    )]
    async fn write_status(
        &self,
        Parameters(input): Parameters<WriteStatusInput>,
    ) -> Result<String, McpError> {
        tokio::task::spawn_blocking(move || {
            let project = project::resolve(&input.working_directory)
                .map_err(|message| McpError::invalid_params(message, None))?;
            let path =
                paths::status_path().map_err(|error| McpError::internal_error(error, None))?;
            let (_, points) = StatusStore::new(path)
                .upsert_with_resolved_repository_result(
                    project.folder_name,
                    input.session_id,
                    project.branch,
                    input.points.into_iter().map(Into::into).collect(),
                    project.repository,
                )
                .map_err(map_status_error)?;
            let points: Vec<AgentPoint> = points.into_iter().map(Into::into).collect();
            serde_json::to_string(&points)
                .map_err(|error| McpError::internal_error(error.to_string(), None))
        })
        .await
        .map_err(|error| McpError::internal_error(format!("Status worker failed: {error}"), None))?
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for StatusServer {}

pub fn run() -> Result<(), String> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?
        .block_on(async {
            StatusServer::new()
                .serve(rmcp::transport::stdio())
                .await
                .map_err(|error| error.to_string())?
                .waiting()
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_blank_branch_can_be_saved_and_read() {
        let temp = tempfile::tempdir().unwrap();
        let project = project::resolve(temp.path().to_str().unwrap()).unwrap();
        assert!(project.branch.is_empty());
        let store = StatusStore::new(temp.path().join("status.json"));
        let point: AgentPoint =
            serde_json::from_value(serde_json::json!({"text": "working"})).unwrap();
        store
            .upsert_with_resolved_repository_result(
                project.folder_name.clone(),
                "session".into(),
                project.branch,
                vec![point.into()],
                project.repository,
            )
            .unwrap();
        let entries = store.read_project(&project.folder_name, "session").unwrap();
        assert_eq!(entries[0].branch, "");
        assert_eq!(entries[0].points[0].text, "working");
        for branch in ["main\nother", "main\rother"] {
            assert!(matches!(
                store.upsert(
                    project.folder_name.clone(),
                    "session".into(),
                    branch.into(),
                    entries[0].points.clone(),
                ),
                Err(StatusError::InvalidInput(_))
            ));
        }
        let mut document = store.read().unwrap();
        document.entries[0].branch = "main\nother".into();
        assert!(crate::status::domain::validate(&document).is_err());
    }

    #[test]
    fn saved_points_support_updates_without_another_read() {
        let temp = tempfile::tempdir().unwrap();
        let store = StatusStore::new(temp.path().join("status.json"));
        let write = |points: Vec<Point>, repository| {
            store
                .upsert_with_repository_result(
                    "project".into(),
                    "session".into(),
                    "main".into(),
                    points,
                    repository,
                )
                .unwrap()
                .1
        };
        let point: AgentPoint =
            serde_json::from_value(serde_json::json!({"text": "working"})).unwrap();
        let first = write(vec![point.into()], Some("owner/repo".into()));
        let id = first[0].id.clone();
        let timestamp = first[0].submitted_at.clone();
        store
            .set_point_pinned(
                "project",
                Some("session"),
                "main",
                &store.read().unwrap().entries[0].updated_at,
                id.as_deref().unwrap(),
                true,
            )
            .unwrap();
        let point: AgentPoint =
            serde_json::from_value(serde_json::json!({"text": "next"})).unwrap();
        let saved = write(vec![point.into()], None);
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].text, "next");
        let mut next = AgentPoint::from(saved[0].clone());
        next.done = true;
        let next_id = next.id.clone();
        let next_timestamp = saved[0].submitted_at.clone();
        let saved = write(vec![next.into()], None);
        assert_eq!(saved[0].id, next_id);
        assert_eq!(saved[0].submitted_at, next_timestamp);
        let document = store.read().unwrap();
        assert_eq!(
            document.entries[0].repository.as_deref(),
            Some("owner/repo")
        );
        let pinned = document.entries[0]
            .points
            .iter()
            .find(|point| point.pinned)
            .unwrap();
        assert_eq!(pinned.id, id);
        assert_eq!(pinned.submitted_at, timestamp);
    }

    #[test]
    fn agent_protocol_excludes_pin_state() {
        let input = serde_json::json!({"text": "activity", "id": "p-1"});
        let point: AgentPoint = serde_json::from_value(input.clone()).unwrap();
        let stored: Point = point.into();
        assert!(!stored.pinned);
        let output = serde_json::to_value(AgentPoint::from(stored)).unwrap();
        assert!(output.get("pinned").is_none());
        assert_eq!(output["id"], "p-1");

        for pinned in [false, true] {
            let mut value = input.clone();
            value["pinned"] = serde_json::json!(pinned);
            assert!(serde_json::from_value::<AgentPoint>(value).is_err());
        }
        let schema = serde_json::to_value(schemars::schema_for!(WriteStatusInput)).unwrap();
        assert!(!schema.to_string().contains("pinned"));

        assert!(output.get("submitted_at").is_none());
        assert!(!schema.to_string().contains("submitted_at"));
    }

    #[test]
    fn tool_inputs_accept_directory_and_reject_agent_metadata() {
        let read = serde_json::json!({"working_directory": "/project", "session_id": "session"});
        assert!(serde_json::from_value::<ReadStatusInput>(read.clone()).is_ok());
        let mut write = read.clone();
        write["points"] = serde_json::json!([{"text": "working"}]);
        assert!(serde_json::from_value::<WriteStatusInput>(write.clone()).is_ok());
        for field in ["folder_name", "branch", "repository"] {
            let mut value = write.clone();
            value[field] = serde_json::json!("override");
            assert!(serde_json::from_value::<WriteStatusInput>(value).is_err());
            let mut value = read.clone();
            value[field] = serde_json::json!("override");
            assert!(serde_json::from_value::<ReadStatusInput>(value).is_err());
        }
        for schema in [
            schemars::schema_for!(ReadStatusInput),
            schemars::schema_for!(WriteStatusInput),
        ] {
            let schema = serde_json::to_value(schema).unwrap();
            assert!(schema["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field == "working_directory"));
            for field in ["folder_name", "branch", "repository"] {
                assert!(schema["properties"].get(field).is_none());
            }
        }
    }

    #[test]
    fn resolved_metadata_updates_existing_session_and_clears_removed_origin() {
        let temp = tempfile::tempdir().unwrap();
        let store = StatusStore::new(temp.path().join("status.json"));
        let save = |repository| {
            let point: AgentPoint =
                serde_json::from_value(serde_json::json!({"text": "working"})).unwrap();
            store
                .upsert_with_resolved_repository_result(
                    "project".into(),
                    "session".into(),
                    "main".into(),
                    vec![point.into()],
                    repository,
                )
                .unwrap();
        };
        save(None);
        save(Some("owner/repo".into()));
        let entries = store.read_project("project", "session").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].repository.as_deref(), Some("owner/repo"));
        save(None);
        assert_eq!(
            store.read_project("project", "session").unwrap()[0].repository,
            None
        );
    }

    #[test]
    fn both_tools_require_a_string_session_id() {
        for value in [
            serde_json::json!({"working_directory": "/project"}),
            serde_json::json!({"working_directory": "/project", "session_id": null}),
        ] {
            assert!(serde_json::from_value::<ReadStatusInput>(value.clone()).is_err());
            let mut write = value;
            write["points"] = serde_json::json!([{"text": "activity"}]);
            assert!(serde_json::from_value::<WriteStatusInput>(write).is_err());
        }
        for schema in [
            schemars::schema_for!(ReadStatusInput),
            schemars::schema_for!(WriteStatusInput),
        ] {
            let value = serde_json::to_value(schema).unwrap();
            assert!(value["required"]
                .as_array()
                .unwrap()
                .iter()
                .any(|field| field == "session_id"));
            assert_eq!(value["properties"]["session_id"]["type"], "string");
        }
    }
}
