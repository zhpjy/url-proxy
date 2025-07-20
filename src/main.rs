mod proxy;

use axum::{
    extract::{Path, Request},
    http::StatusCode,
    response::{IntoResponse},
    routing::any,
    Router,
};
use std::env;
use std::sync::LazyLock;
use tracing::{info, warn};

static PASSWORD: LazyLock<String> = LazyLock::new(|| {
    env::var("PASSWORD").unwrap_or_default()
});

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    info!("Starting server on 127.0.0.1:3000");
    
    // build our application with a route
    let app = Router::new().route("/{*path}", any(handler));

    // add a fallback service for handling routes to unknown paths
    let app = app.fallback(handler_404);

    // run it
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}

async fn handler(Path(path): Path<String>, request: Request) -> impl IntoResponse {
    if PASSWORD.is_empty() || !path.starts_with(&*PASSWORD) {
        return handler_404().await.into_response();
    }
    
    info!("Authorized access: {}", path);
    
    // Remove password from path
    let new_path = &path[PASSWORD.len()..];
    let new_path = (if new_path.is_empty() { "/" } else { new_path }).trim_start_matches('/');
    let has_protocol = new_path.starts_with("http://") || new_path.starts_with("https://");
    let new_path_protocol = if has_protocol { new_path } else { &Path(format!("https://{}", new_path))};

    info!("new path: {}", new_path_protocol);

    
    // Extract request components
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();
    let body = request.into_body();
    
    // Call proxy function
    proxy::proxy_request(method, new_path_protocol, uri.query(), headers, body).await.into_response()
}

async fn handler_404() -> impl IntoResponse {
    warn!("404 - Route not found");
    (StatusCode::NOT_FOUND, "nothing to see here")
}
