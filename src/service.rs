use crate::client::{ElmClient, Package};
use rmcp::{
    model::{
        CallToolResult, Content, Implementation, InitializeRequestParam, InitializeResult,
        ProtocolVersion, ServerCapabilities, ServerInfo,
    },
    service::RequestContext,
    tool, Error as McpError, RoleServer, ServerHandler,
};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct ElmService {
    packages: Arc<Mutex<Option<Vec<Package>>>>,
    client: ElmClient,
}

const ELM_PROJECT_FOLDER: Option<&str> = std::option_env!("ELM_PROJECT_FOLDER");

#[tool(tool_box)]
impl ElmService {
    pub fn new() -> Self {
        Self {
            packages: Default::default(),
            client: ElmClient::new(),
        }
    }

    #[tool(description = "Gets the latest available package version for <USERNAME>/<PACKAGE>")]
    async fn get_latest_package_version(
        &self,
        #[tool(param)] username: String,
        #[tool(param)] package: String,
    ) -> Result<CallToolResult, McpError> {
        let latest_version = self
            .client
            .get_latest_package_version(&username, &package)
            .await?;
        Ok(CallToolResult::success(vec![Content::text(latest_version)]))
    }

    #[tool(description = "Gets the docs for a specified Elm package")]
    async fn get_docs(
        &self,
        #[tool(param)] username: String,
        #[tool(param)] package: String,
        #[tool(param)] version: String,
    ) -> Result<CallToolResult, McpError> {
        let docs = self.client.get_docs(&username, &package, &version).await?;
        let out = Content::json(docs)?;
        Ok(CallToolResult::success(vec![out]))
    }

    #[tool(
        description = "Search Elm packages by package name. Allowed characters: digits (0-9), lowercase letters (a-z), hyphen (-)"
    )]
    async fn search_packages(
        &self,
        #[tool(param)] search_string: String,
    ) -> Result<CallToolResult, McpError> {
        let string_is_valid = validate_string(&search_string);

        if !string_is_valid {
            return Err(McpError::internal_error(
                "Allowed characters: digits (0-9), lowercase letters (a-z), hyphen (-)",
                None,
            ));
        }

        let mut lock = self.packages.lock().await;
        let data = match &*lock {
            Some(cache) => cache.clone(),
            None => {
                let data = self.client.fetch_all_packages().await?;
                *lock = Some(data.clone());
                data
            }
        };
        let val = search_string.to_lowercase();
        let results: Vec<_> = data
            .into_iter()
            .filter(|pkg| pkg.name.contains(&val))
            .collect();
        let out = Content::json(results)?;
        Ok(CallToolResult::success(vec![out]))
    }

    #[tool(description = "Compiles and validates the current Elm project")]
    async fn validate(&self) -> Result<CallToolResult, McpError> {
        let path = get_project_folder()?;
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
            let err_data: serde_json::Value = serde_json::from_str(&err)
                .map_err(|_| McpError::internal_error("Compile error serialize fail", None))?;
            let out = Content::json(err_data)?;
            Ok(CallToolResult::success(vec![out]))
        }
    }

    #[tool(description = "Adds a package to current Elm project")]
    async fn add_package(
        &self,
        #[tool(param)] username: String,
        #[tool(param)] package: String,
    ) -> Result<CallToolResult, McpError> {
        let path = get_project_folder()?;
        let package = validate_package(&username, &package)?;
        let output = std::process::Command::new("elm-json")
            .arg("install")
            .arg("--yes")
            .arg(package)
            .current_dir(&path)
            .output()
            .map_err(|e| McpError::internal_error(format!("Failed to install: {}", e), None))?;
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
        #[tool(param)] username: String,
        #[tool(param)] package: String,
    ) -> Result<CallToolResult, McpError> {
        let path = get_project_folder()?;
        let package = validate_package(&username, &package)?;
        let output = std::process::Command::new("elm-json")
            .arg("uninstall")
            .arg("--yes")
            .arg(package)
            .current_dir(&path)
            .output()
            .map_err(|e| McpError::internal_error(format!("Failed to uninstall: {}", e), None))?;
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

fn validate_string(val: &str) -> bool {
    val.chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn validate_package(username: &str, package: &str) -> Result<String, McpError> {
    if !validate_string(username) && !validate_string(package) {
        return Err(McpError::internal_error(
            "Allowed characters: digits (0-9), lowercase letters (a-z), hyphen (-)",
            None,
        ));
    }
    Ok(format!("{username}/{package}"))
}

fn get_project_folder() -> Result<String, McpError> {
    let Some(path) = ELM_PROJECT_FOLDER else {
        return Err(McpError::internal_error("Missing project path", None));
    };
    Ok(path.to_string())
}
