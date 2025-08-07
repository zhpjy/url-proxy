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
use regex::{self, Regex};

static PASSWORD: LazyLock<String> = LazyLock::new(|| {
    env::var("PASSWORD").unwrap_or_default()
});

static RE_HTTP: LazyLock<Regex> = LazyLock::new(|| {
     regex::Regex::new(r"^http:\/{1,2}").unwrap()
});

static RE_HTTPS: LazyLock<Regex> = LazyLock::new(|| {
     regex::Regex::new(r"^https:\/{1,2}").unwrap()
});

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    //check if password is set
    if PASSWORD.is_empty() {
        warn!("PASSWORD environment variable is required.");
        std::process::exit(1);
    }
    
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let bind_addr = format!("{}:{}", host, port);
    
    info!("Starting server on {}", bind_addr);
    
    // build our application with a route
    let app = Router::new().route("/{*path}", any(handler));

    // add a fallback service for handling routes to unknown paths
    let app = app.fallback(handler_404);

    // run it
    let listener = tokio::net::TcpListener::bind(&bind_addr)
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
    // Replace leading ^http:\/{1,2} with http:// using regex
    let new_path = RE_HTTPS.replace(RE_HTTP.replace(new_path, "http://").to_string().as_str(), "https://").to_string();
    // Some ingress will combine // as /
    let has_protocol = new_path.starts_with("http:/") || new_path.starts_with("https:/");
    let new_path_protocol = if has_protocol { new_path } else { format!("https://{}", new_path)};

    info!("new path: {}", new_path_protocol);

    
    // Extract request components
    let method = request.method().clone();
    let uri = request.uri().clone();
    let headers = request.headers().clone();
    let body = request.into_body();
    
    // Call proxy function
    proxy::proxy_request(method, new_path_protocol.as_str(), uri.query(), headers, body).await.into_response()
}

async fn handler_404() -> impl IntoResponse {
    warn!("404 - Route not found");
    (StatusCode::NOT_FOUND, "nothing to see here")
}
