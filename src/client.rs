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
    ) -> anyhow::Result<String> {
        let releases: HashMap<String, u32> = self
            .client
            .get(format!(
                "https://package.elm-lang.org/packages/{}/{}/releases.json",
                username, package
            ))
            .send()
            .await
            .map_err(fail("PACKAGE_FETCH_FAIL"))?
            .json()
            .await
            .map_err(fail("PACKAGE_DECODE_FAIL"))?;

        releases
            .iter()
            .max_by_key(|&(_, timestamp)| timestamp)
            .map(|(version, _)| version.clone())
            .ok_or(anyhow::anyhow!("PACKAGE_LIST_EMPTY"))
    }

    pub async fn get_docs(
        &self,
        username: &str,
        package: &str,
        version: &str,
    ) -> anyhow::Result<serde_json::Value> {
        let res = self
            .client
            .get(format!(
                "https://package.elm-lang.org/packages/{}/{}/{}/docs.json",
                username, package, version
            ))
            .send()
            .await
            .map_err(fail("DOCS_FETCH_FAIL"))?
            .json()
            .await
            .map_err(fail("DOCS_DECODE_FAIL"))?;

        Ok(res)
    }

    pub async fn fetch_all_packages(&self) -> anyhow::Result<Vec<Package>> {
        let res = self
            .client
            .get("https://package.elm-lang.org/search.json")
            .send()
            .await
            .map_err(fail("PACKAGES_FETCH_FAIL"))?
            .json()
            .await
            .map_err(fail("PACKAGES_DECODE_FAIL"))?;

        Ok(res)
    }
}

fn fail<E: std::fmt::Debug>(tag: &str) -> impl Fn(E) -> anyhow::Error {
    move |err: E| {
        eprintln!("{}:\n{:#?}", tag, err);
        anyhow::anyhow!("{tag}")
    }
}
