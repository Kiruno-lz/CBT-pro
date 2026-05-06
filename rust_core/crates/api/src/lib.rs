//! CBT-Pro API Gateway crate.
//!
//! Provides Axum-based REST API and WebSocket gateway for the backtest engine.

pub mod server;
pub mod websocket;

use axum::serve;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;

pub use server::create_rest_router;
pub use server::{AppState, AppStateInner};

/// Run both REST and WebSocket servers.
pub async fn run_api(rest_addr: SocketAddr, ws_addr: SocketAddr) -> Result<(), ApiError> {
    let database_url = std::env::var("DATABASE_URL").ok();
    let data_provider = if let Some(ref url) = database_url {
        match data_pipeline::backtest::BacktestDataProvider::from_config(url, "binance", 1000).await
        {
            Ok(provider) => {
                info!("BacktestDataProvider initialized successfully");
                Some(provider)
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to initialize BacktestDataProvider ({}), falling back to synthetic data",
                    e
                );
                None
            }
        }
    } else {
        tracing::warn!("DATABASE_URL not set, using synthetic data for backtests");
        None
    };

    let state: AppState = Arc::new(Mutex::new(AppStateInner {
        engines: HashMap::new(),
        data_provider,
        data_provider_exchange: None,
    }));

    // REST server
    let rest_app = create_rest_router().with_state(state.clone());
    let rest_listener = tokio::net::TcpListener::bind(rest_addr).await?;
    info!("REST API listening on {}", rest_addr);

    // WebSocket server
    let ws_app = axum::Router::new()
        .route("/ws", axum::routing::get(websocket::ws_handler))
        .with_state(state.clone());
    let ws_listener = tokio::net::TcpListener::bind(ws_addr).await?;
    info!("WebSocket listening on {}", ws_addr);

    tokio::select! {
        result = serve(rest_listener, rest_app) => {
            result?;
        }
        result = serve(ws_listener, ws_app) => {
            result?;
        }
    }

    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("HTTP error: {0}")]
    Http(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
