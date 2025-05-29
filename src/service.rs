use rmcp::{model::*, service::RequestContext, tool, Error as McpError, RoleServer, ServerHandler};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct ElmService {
    packages: Arc<Mutex<Option<serde_json::Value>>>,
}

const ELM_PROJECT_FOLDER: Option<&str> = std::option_env!("ELM_PROJECT_FOLDER");

#[tool(tool_box)]
impl ElmService {
    pub fn new() -> Self {
        Self {
            packages: Default::default(),
        }
    }

    #[tool(description = "Gets the latest available package version for <USERNAME>/<PACKAGE>")]
    async fn get_latest_package_version(
        &self,
        #[tool(param)] username: String,
        #[tool(param)] package: String,
    ) -> Result<CallToolResult, McpError> {
        let releases: HashMap<String, u32> = reqwest::get(format!(
            "https://package.elm-lang.org/packages/{}/{}/releases.json",
            username, package
        ))
        .await
        .map_err(|e| McpError::internal_error(format!("Package fetch fail: {}", e), None))?
        .json()
        .await
        .map_err(|e| McpError::internal_error(format!("Package decode fail: {}", e), None))?;
        let latest_version = releases
            .iter()
            .max_by_key(|&(_, timestamp)| timestamp)
            .map(|(version, _)| version)
            .ok_or(McpError::internal_error("Package list empty", None))?;
        Ok(CallToolResult::success(vec![Content::text(
            latest_version.to_string(),
        )]))
    }

    #[tool(description = "Gets the docs for a specified Elm package")]
    async fn get_docs(
        &self,
        #[tool(param)] username: String,
        #[tool(param)] package: String,
        #[tool(param)] version: String,
    ) -> Result<CallToolResult, McpError> {
        let docs: serde_json::Value = reqwest::get(format!(
            "https://package.elm-lang.org/packages/{}/{}/{}/docs.json",
            username, package, version
        ))
        .await
        .map_err(|e| McpError::internal_error(format!("Package fetch fail: {}", e), None))?
        .json()
        .await
        .map_err(|e| McpError::internal_error(format!("Package decode fail: {}", e), None))?;
        let out = Content::json(docs)?;
        Ok(CallToolResult::success(vec![out]))
    }

    #[tool(description = "Gets all available Elm packages")]
    async fn get_packages(&self) -> Result<CallToolResult, McpError> {
        let mut lock = self.packages.lock().await;
        let data = match lock.clone() {
            Some(cache) => cache,
            None => {
                let data: serde_json::Value =
                    reqwest::get("https://package.elm-lang.org/search.json")
                        .await
                        .map_err(|e| {
                            McpError::internal_error(format!("Packages fetch fail: {}", e), None)
                        })?
                        .json()
                        .await
                        .map_err(|e| {
                            McpError::internal_error(format!("Packages decode fail: {}", e), None)
                        })?;
                *lock = Some(data.clone());
                data
            }
        };
        let out = Content::json(data)?;
        Ok(CallToolResult::success(vec![out]))
    }

    #[tool(description = "Compiles and validates the current Elm project")]
    async fn validate(&self) -> Result<CallToolResult, McpError> {
        let Some(path) = ELM_PROJECT_FOLDER else {
            return Err(McpError::internal_error("Missing project path", None));
        };
        let output = std::process::Command::new("elm")
            .arg("make")
            .arg("--output=/dev/null")
            .arg("--report=json")
            .arg("./src/Main.elm")
            .current_dir(&path)
            .output()
            .map_err(|e| {
                McpError::internal_error(format!("Failed to run Elm compiler: {}", e), None)
            })?;

        let err = String::from_utf8_lossy(&output.stderr);

        if err.is_empty() {
            Ok(CallToolResult::success(vec![Content::text(
                "OK".to_string(),
            )]))
        } else {
            let err_data: serde_json::Value =
                serde_json::from_str(&err)
                    .ok()
                    .ok_or(McpError::internal_error(
                        "Compile error serialize fail",
                        None,
                    ))?;
            let out = Content::json(err_data)?;
            Ok(CallToolResult::success(vec![out]))
        }
    }
}

#[tool(tool_box)]
impl ServerHandler for ElmService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "This server provides a variety of tools that interact with the Elm ecosystem."
                    .to_string(),
            ),
        }
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        if let Some(http_request_part) = context.extensions.get::<axum::http::request::Parts>() {
            let initialize_headers = &http_request_part.headers;
            let initialize_uri = &http_request_part.uri;
            tracing::info!(?initialize_headers, %initialize_uri, "initialize from http server");
        }
        Ok(self.get_info())
    }
}
