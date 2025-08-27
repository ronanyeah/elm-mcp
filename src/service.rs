use crate::client::{ElmClient, Package};
use rmcp::{
    handler::server::tool::{Parameters, ToolRouter},
    model::{
        CallToolResult, Content, Implementation, InitializeRequestParam, InitializeResult,
        ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    schemars,
    service::RequestContext,
    tool, tool_handler, tool_router, RoleServer, ServerHandler,
};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct ElmService {
    packages: Arc<Mutex<Option<Vec<Package>>>>,
    client: ElmClient,
    project_folder: String,
    entry_file: String,
    tool_router: ToolRouter<ElmService>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PackageRequest {
    pub package: String,
    pub username: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct DocsRequest {
    pub package: String,
    pub username: String,
    pub version: String,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchRequest {
    pub query: String,
}

#[tool_router]
impl ElmService {
    pub fn new(project_folder: &str, entry_file: &str) -> Self {
        Self {
            packages: Default::default(),
            client: ElmClient::new(),
            project_folder: project_folder.to_string(),
            entry_file: entry_file.to_string(),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(description = "Gets the latest available package version for <USERNAME>/<PACKAGE>")]
    async fn get_latest_package_version(
        &self,
        Parameters(PackageRequest { package, username }): Parameters<PackageRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let latest_version = self
            .client
            .get_latest_package_version(&username, &package)
            .await
            .map_err(convert_error)?;
        Ok(CallToolResult::success(vec![Content::text(latest_version)]))
    }

    #[tool(description = "Gets the docs for a specified Elm package")]
    async fn get_docs(
        &self,
        Parameters(DocsRequest {
            package,
            username,
            version,
        }): Parameters<DocsRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let docs = self
            .client
            .get_docs(&username, &package, &version)
            .await
            .map_err(convert_error)?;
        let out = Content::json(docs)?;
        Ok(CallToolResult::success(vec![out]))
    }

    #[tool(
        description = "Search Elm packages by package name. Allowed characters: digits (0-9), lowercase letters (a-z), hyphen (-)"
    )]
    async fn search_packages(
        &self,
        Parameters(SearchRequest { query }): Parameters<SearchRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let string_is_valid = validate_string(&query);

        if !string_is_valid {
            return Err(rmcp::ErrorData::internal_error(
                "Allowed characters: digits (0-9), lowercase letters (a-z), hyphen (-)",
                None,
            ));
        }

        let mut lock = self.packages.lock().await;
        let data = match &*lock {
            Some(cache) => cache.clone(),
            None => {
                let data = self
                    .client
                    .fetch_all_packages()
                    .await
                    .map_err(convert_error)?;
                *lock = Some(data.clone());
                data
            }
        };
        let val = query.to_lowercase();
        let results: Vec<_> = data
            .into_iter()
            .filter(|pkg| pkg.name.contains(&val))
            .collect();
        let out = Content::json(results)?;
        Ok(CallToolResult::success(vec![out]))
    }

    #[tool(description = "Compiles and validates the current Elm project")]
    async fn validate(&self) -> Result<CallToolResult, rmcp::ErrorData> {
        let output = std::process::Command::new("elm")
            .arg("make")
            .arg("--output=/dev/null")
            .arg("--report=json")
            .arg(&self.entry_file)
            .current_dir(&self.project_folder)
            .output()
            .map_err(|e| {
                rmcp::ErrorData::internal_error(format!("Failed to run Elm compiler: {}", e), None)
            })?;

        let err = String::from_utf8_lossy(&output.stderr);
        if err.is_empty() {
            Ok(CallToolResult::success(vec![Content::text(
                "OK".to_string(),
            )]))
        } else {
            let err_data: serde_json::Value = serde_json::from_str(&err).map_err(|_| {
                rmcp::ErrorData::internal_error("Compile error serialize fail", None)
            })?;
            let out = Content::json(err_data)?;
            Ok(CallToolResult::error(vec![out]))
        }
    }

    #[tool(description = "Adds a package to current Elm project")]
    async fn add_package(
        &self,
        Parameters(PackageRequest { package, username }): Parameters<PackageRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let package = validate_package(&username, &package)?;
        let output = std::process::Command::new("elm-json")
            .arg("install")
            .arg("--yes")
            .arg(package)
            .current_dir(&self.project_folder)
            .output()
            .map_err(|e| {
                rmcp::ErrorData::internal_error(format!("Failed to install: {}", e), None)
            })?;
        let err = String::from_utf8_lossy(&output.stderr);
        if err.is_empty() {
            Ok(CallToolResult::success(vec![Content::text(
                "OK".to_string(),
            )]))
        } else {
            let out = Content::text(err);
            Ok(CallToolResult::success(vec![out]))
        }
    }

    #[tool(description = "Removes a package from current Elm project")]
    async fn remove_package(
        &self,
        Parameters(PackageRequest { package, username }): Parameters<PackageRequest>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        let package = validate_package(&username, &package)?;
        let output = std::process::Command::new("elm-json")
            .arg("uninstall")
            .arg("--yes")
            .arg(package)
            .current_dir(&self.project_folder)
            .output()
            .map_err(|e| {
                rmcp::ErrorData::internal_error(format!("Failed to uninstall: {}", e), None)
            })?;
        let err = String::from_utf8_lossy(&output.stderr);
        if err.is_empty() {
            Ok(CallToolResult::success(vec![Content::text(
                "OK".to_string(),
            )]))
        } else {
            let out = Content::text(err);
            Ok(CallToolResult::success(vec![out]))
        }
    }
}

#[tool_handler]
impl ServerHandler for ElmService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2025_06_18,
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
    ) -> Result<InitializeResult, rmcp::ErrorData> {
        if let Some(http_request_part) = context.extensions.get::<axum::http::request::Parts>() {
            let initialize_headers = &http_request_part.headers;
            let initialize_uri = &http_request_part.uri;
            tracing::info!(?initialize_headers, %initialize_uri, "initialize from http server");
        }
        Ok(self.get_info())
    }
}

fn validate_string(val: &str) -> bool {
    val.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn validate_package(username: &str, package: &str) -> Result<String, rmcp::ErrorData> {
    if !validate_string(username) && !validate_string(package) {
        return Err(rmcp::ErrorData::internal_error(
            "Allowed characters: digits (0-9), lowercase letters (a-z), hyphen (-)",
            None,
        ));
    }
    Ok(format!("{username}/{package}"))
}

fn convert_error(err: anyhow::Error) -> rmcp::ErrorData {
    rmcp::ErrorData::internal_error(err.to_string(), None)
}
