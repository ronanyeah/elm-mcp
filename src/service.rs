use anyhow::Result;
use rmcp::{model::*, service::RequestContext, tool, Error as McpError, RoleServer, ServerHandler};
use std::collections::HashMap;

#[derive(Clone)]
pub struct ElmService;

#[tool(tool_box)]
impl ElmService {
    pub fn new() -> Self {
        Self
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
        .unwrap()
        .json()
        .await
        .unwrap();
        let latest_version = releases
            .iter()
            .max_by_key(|&(_, timestamp)| timestamp)
            .map(|(version, _)| version)
            .unwrap();
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
        .unwrap()
        .json()
        .await
        .unwrap();
        let out = Content::json(docs).unwrap();
        Ok(CallToolResult::success(vec![out]))
    }

    #[tool(description = "Gets all available Elm packages")]
    async fn get_packages(&self) -> Result<CallToolResult, McpError> {
        let docs: serde_json::Value = reqwest::get("https://package.elm-lang.org/search.json")
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let out = Content::json(docs).unwrap();
        Ok(CallToolResult::success(vec![out]))
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
