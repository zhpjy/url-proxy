use axum::{
    body::Body,
    http::{HeaderMap, Method, StatusCode, },
    response::{IntoResponse, Response},
};
use tracing::{error, info};

pub async fn proxy_request(
    method: Method,
    new_path: &str,
    query: Option<&str>,
    headers: HeaderMap,
    body: Body,
) -> impl IntoResponse {
    let query_string = query.map(|q| format!("?{}", q)).unwrap_or_default();
    let target_url = format!("{}{}", new_path, query_string);
    
    info!("Proxying {} request to: {}", method, target_url);
    
    // Create HTTP client
    let client = reqwest::Client::new();
    
    // Build the request
    let mut request_builder = client.request(method.as_str().parse().unwrap(), &target_url);
    
    // Copy headers (excluding host and other connection-specific headers)
    for (name, value) in headers.iter() {
        if !["host", "connection", "content-length"].contains(&name.as_str().to_lowercase().as_str()) {
            if let Ok(value_str) = value.to_str() {
                request_builder = request_builder.header(name.as_str(), value_str);
            }
        }
    }
    
    // Add body if present
    let body_bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            error!("Failed to read request body: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read body").into_response();
        }
    };
    
    if !body_bytes.is_empty() {
        request_builder = request_builder.body(body_bytes.to_vec());
    }
    
    // Execute the request
    match request_builder.send().await {
        Ok(response) => {
            let status = response.status();
            let headers = response.headers().clone();
            let body = match response.bytes().await {
                Ok(bytes) => bytes,
                Err(e) => {
                    error!("Failed to read response body: {}", e);
                    return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read response").into_response();
                }
            };
            
            // Build response
            let mut resp = Response::builder().status(status.as_u16());
            
            // Copy response headers
            for (name, value) in headers.iter() {
                resp = resp.header(name.as_str(), value.as_bytes());
            }
            
            resp.body(Body::from(body)).unwrap().into_response()
        }
        Err(e) => {
            error!("Proxy request failed: {}", e);
            (StatusCode::BAD_GATEWAY, "Proxy request failed").into_response()
        }
    }
}
