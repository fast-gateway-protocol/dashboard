//! FGP Dashboard - Web UI for monitoring daemon services.
//!
//! # Usage
//!
//! ```bash
//! fgp-dashboard                     # Start on default port 8765
//! fgp-dashboard --port 9000         # Custom port
//! fgp-dashboard --open              # Open browser automatically
//! ```

mod api;

use anyhow::Result;
use axum::{
    routing::{get, post},
    Router,
};
use clap::Parser;
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// FGP Dashboard - Web UI for monitoring daemon services
#[derive(Parser)]
#[command(name = "fgp-dashboard")]
#[command(author, version, about)]
struct Args {
    /// Port to listen on
    #[arg(short, long, default_value = "8765")]
    port: u16,

    /// Open browser automatically
    #[arg(short, long)]
    open: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "fgp_dashboard=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    // Build router
    let app = Router::new()
        // API routes
        .route("/api/services", get(api::list_services))
        .route("/api/health/{service}", get(api::service_health))
        .route("/api/start/{service}", post(api::start_service))
        .route("/api/stop/{service}", post(api::stop_service))
        // Static dashboard
        .route("/", get(api::serve_dashboard))
        // CORS for local development
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    // Bind to localhost only (security)
    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    let url = format!("http://localhost:{}", args.port);

    tracing::info!("FGP Dashboard starting at {}", url);

    // Open browser if requested
    if args.open {
        tracing::info!("Opening browser...");
        let _ = open::that(&url);
    }

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
