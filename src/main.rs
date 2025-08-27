use elm_mcp::service::ElmService;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpService,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(serde::Deserialize)]
struct Env {
    port: u16,
    project_folder: String,
    entry_file: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let env = envy::from_env::<Env>()?;

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "debug".to_string().into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let bind_address = format!("127.0.0.1:{}", env.port);

    let entry_file = env.entry_file.unwrap_or("./src/Main.elm".to_string());

    println!("Project folder: {}", env.project_folder);
    println!("Entry file: {}", entry_file);

    let service = StreamableHttpService::new(
        move || Ok(ElmService::new(&env.project_folder, &entry_file)),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let router = axum::Router::new().nest_service("/mcp", service);
    let tcp_listener = tokio::net::TcpListener::bind(bind_address).await?;
    axum::serve(tcp_listener, router).await?;

    Ok(())
}
