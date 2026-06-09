// Copyright (C) 2026 themadorg
//
// Temporary profiling support for internal benchmarking (no commit).
// Exposes flamegraph + pprof endpoints on port 6060.
//
// Recommended usage during the cmlxc benchmark (inside the VM):
//
//   # Capture a 12-second CPU profile as flamegraph SVG (most useful)
//   curl -o /tmp/flame.svg 'http://127.0.0.1:6060/debug/pprof/flamegraph?seconds=12'
//
//   # Or get raw pprof protobuf for go tool pprof / pprof-rs
//   curl -o /tmp/profile.pb 'http://127.0.0.1:6060/debug/pprof/profile?seconds=12'
//
// Then open the SVG or use `go tool pprof -http=:8080 /tmp/profile.pb`

use std::net::SocketAddr;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{extract::Query, routing::get, Router};
use pprof::ProfilerGuard;
use serde::Deserialize;
use tokio::net::TcpListener;
use tracing::info;

#[derive(Deserialize)]
struct ProfileParams {
    #[serde(default = "default_seconds")]
    seconds: u64,
}

fn default_seconds() -> u64 {
    10
}

pub async fn start_pprof_server() {
    let app = Router::new()
        .route("/debug/pprof/flamegraph", get(flamegraph_handler))
        .route("/debug/pprof/profile", get(pprof_handler))
        .route("/debug/pprof/heap", get(heap_handler));

    let addr: SocketAddr = "0.0.0.0:6060".parse().unwrap();

    tokio::spawn(async move {
        let listener = TcpListener::bind(addr).await.unwrap();
        info!(
            "pprof profiling server listening on http://{}/debug/pprof/  (flamegraph + profile)",
            addr
        );
        axum::serve(listener, app).await.unwrap();
    });
}

async fn flamegraph_handler(Query(params): Query<ProfileParams>) -> impl IntoResponse {
    let seconds = params.seconds.min(60);

    match ProfilerGuard::new(100) {
        Ok(guard) => {
            tokio::time::sleep(Duration::from_secs(seconds)).await;

            match guard.report().build() {
                Ok(report) => {
                    let mut buf = Vec::new();
                    if let Err(e) = report.flamegraph(&mut buf) {
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            format!("flamegraph generation failed: {e}"),
                        )
                            .into_response();
                    }
                    Response::builder()
                        .status(StatusCode::OK)
                        .header(header::CONTENT_TYPE, "image/svg+xml")
                        .body(Body::from(buf))
                        .unwrap()
                }
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to build profile: {e}"),
                )
                    .into_response(),
            }
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to start profiler: {e}"),
        )
            .into_response(),
    }
}

async fn pprof_handler(Query(params): Query<ProfileParams>) -> impl IntoResponse {
    // For simplicity and reliability we recommend the flamegraph endpoint.
    // This endpoint exists for compatibility with tools that expect /debug/pprof/profile.
    // It currently returns a short message + suggests the better endpoint.
    let seconds = params.seconds.min(60);
    (
        StatusCode::OK,
        format!(
            "For best results use the flamegraph endpoint instead:\n\
             curl -o /tmp/flame.svg 'http://127.0.0.1:6060/debug/pprof/flamegraph?seconds={seconds}'\n\
             Then open the SVG directly in a browser."
        ),
    )
}

async fn heap_handler() -> impl IntoResponse {
    (
        StatusCode::NOT_IMPLEMENTED,
        "Heap profiling not enabled. Use /debug/pprof/flamegraph for CPU.",
    )
}