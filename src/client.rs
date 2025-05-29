use rmcp::Error as McpError;
use std::collections::HashMap;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct Package {
    pub name: String,
    pub summary: String,
    pub license: String,
    pub version: String,
}

#[derive(Clone)]
pub struct ElmClient {
    client: reqwest::Client,
}

impl ElmClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    pub async fn get_latest_package_version(
        &self,
        username: &str,
        package: &str,
    ) -> Result<String, McpError> {
        let releases: HashMap<String, u32> = self
            .client
            .get(format!(
                "https://package.elm-lang.org/packages/{}/{}/releases.json",
                username, package
            ))
            .send()
            .await
            .map_err(|e| McpError::internal_error(format!("Package fetch fail: {}", e), None))?
            .json()
            .await
            .map_err(|e| McpError::internal_error(format!("Package decode fail: {}", e), None))?;

        releases
            .iter()
            .max_by_key(|&(_, timestamp)| timestamp)
            .map(|(version, _)| version.clone())
            .ok_or(McpError::internal_error("Package list empty", None))
    }

    pub async fn get_docs(
        &self,
        username: &str,
        package: &str,
        version: &str,
    ) -> Result<serde_json::Value, McpError> {
        self.client
            .get(format!(
                "https://package.elm-lang.org/packages/{}/{}/{}/docs.json",
                username, package, version
            ))
            .send()
            .await
            .map_err(|e| McpError::internal_error(format!("Docs fetch fail: {}", e), None))?
            .json()
            .await
            .map_err(|e| McpError::internal_error(format!("Docs decode fail: {}", e), None))
    }

    pub async fn fetch_all_packages(&self) -> Result<Vec<Package>, McpError> {
        self.client
            .get("https://package.elm-lang.org/search.json")
            .send()
            .await
            .map_err(|e| McpError::internal_error(format!("Packages fetch fail: {}", e), None))?
            .json()
            .await
            .map_err(|e| McpError::internal_error(format!("Packages decode fail: {}", e), None))
    }
}
