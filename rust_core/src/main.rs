use std::net::SocketAddr;
use api::run_api;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let rest_port: u16 = std::env::var("REST_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .unwrap_or(8080);
    let ws_port: u16 = std::env::var("WS_PORT")
        .unwrap_or_else(|_| "8081".to_string())
        .parse()
        .unwrap_or(8081);

    let rest_addr = SocketAddr::from(([0, 0, 0, 0], rest_port));
    let ws_addr = SocketAddr::from(([0, 0, 0, 0], ws_port));

    run_api(rest_addr, ws_addr).await?;

    Ok(())
}