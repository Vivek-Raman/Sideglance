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
    text: String,
    #[serde(default)]
    done: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    /// Repository owner/name from the Git remote, such as "owner/repo".
    repository: Option<String>,
    /// The current project's folder name, such as "sideglance".
    folder_name: String,
    /// Your stable session ID from the harness.
    session_id: String,
    /// Currently checked out branch.
    branch: String,
    /// Complete revised point list; omitted points are removed.
    points: Vec<AgentPoint>,
}

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadStatusInput {
    /// The current project's folder name, such as "sideglance".
    folder_name: String,
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
        StatusError::InvalidDocument(message) | StatusError::Io(message) => {
            McpError::internal_error(message, None)
        }
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
        let path = paths::status_path()
            .map_err(|error| McpError::internal_error(error.to_string(), None))?;
        let entries = StatusStore::new(path)
            .read_project(&input.folder_name, &input.session_id)
            .map_err(map_status_error)?;
        let points: Vec<AgentPoint> = entries
            .into_iter()
            .flat_map(|entry| entry.points)
            .map(Into::into)
            .collect();
        serde_json::to_string(&points)
            .map_err(|error| McpError::internal_error(error.to_string(), None))
    }

    #[tool(
        description = "Replace your points; omitted points are removed. Returns saved points with IDs for the next update."
    )]
    async fn write_status(
        &self,
        Parameters(input): Parameters<WriteStatusInput>,
    ) -> Result<String, McpError> {
        let path = paths::status_path().map_err(|error| McpError::internal_error(error, None))?;
        let (_, points) = StatusStore::new(path)
            .upsert_with_repository_result(
                input.folder_name.clone(),
                input.session_id,
                input.branch,
                input.points.into_iter().map(Into::into).collect(),
                input.repository,
            )
            .map_err(|error| map_status_error(error))?;
        let points: Vec<AgentPoint> = points.into_iter().map(Into::into).collect();
        serde_json::to_string(&points)
            .map_err(|error| McpError::internal_error(error.to_string(), None))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for StatusServer {}

pub fn run() -> Result<(), String> {
    tokio::runtime::Runtime::new()
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
    fn both_tools_require_a_string_session_id() {
        for value in [
            serde_json::json!({"folder_name": "project"}),
            serde_json::json!({"folder_name": "project", "session_id": null}),
        ] {
            assert!(serde_json::from_value::<ReadStatusInput>(value.clone()).is_err());
            let mut write = value;
            write["branch"] = serde_json::json!("main");
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
