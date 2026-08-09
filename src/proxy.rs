use axum::{
    body::Body,
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Response},
};
use futures_util::TryStreamExt;
use http_body_util::BodyStream;
use std::sync::LazyLock;
use tracing::{error, info};

/// Follow redirects on the proxy server so clients receive the final response.
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()
        .expect("failed to build proxy HTTP client")
});

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

    // Build the request
    let has_request_body = method != Method::GET && method != Method::HEAD;
    let mut request_builder = CLIENT.request(method, &target_url);

    if has_request_body {
        // Convert the axum Body into a stream of bytes for reqwest.
        // BodyStream yields Result<Frame<Bytes>, Error>.
        let body_stream = BodyStream::new(body);
        // Filter out trailer frames and extract bytes from data frames.
        let stream_of_bytes =
            body_stream.try_filter_map(|frame| async move { Ok(frame.into_data().ok()) });
        // Map the error type to what reqwest::Body::wrap_stream expects.
        let stream = stream_of_bytes
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));
        request_builder = request_builder.body(reqwest::Body::wrap_stream(stream));
    }

    // Copy headers, letting reqwest handle connection-level headers.
    // We remove the 'host' header as it's for the proxy server itself.
    let mut new_headers = headers.clone();
    new_headers.remove("host");
    request_builder = request_builder.headers(new_headers);

    // Execute the request
    match request_builder.send().await {
        Ok(response) => {
            let status = response.status();
            let headers = response.headers().clone();

            // Get the response body as a stream
            let stream = response.bytes_stream();
            // Create an axum Body from the stream to send to the client
            let body = Body::from_stream(stream);

            // Build the final response
            let mut resp = Response::builder().status(status);

            // Copy response headers
            // The headers_mut().unwrap() is safe here because the builder is fresh.
            *resp.headers_mut().unwrap() = headers;

            // Create the response and send it
            match resp.body(body) {
                Ok(final_response) => final_response.into_response(),
                Err(e) => {
                    error!("Failed to construct response: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "Failed to construct response",
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            error!("Proxy request failed: {}", e);
            (StatusCode::BAD_GATEWAY, "Proxy request failed").into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::proxy_request;
    use axum::{
        body::Body,
        extract::Path,
        http::{HeaderMap, Method, StatusCode},
        response::IntoResponse,
        routing::get,
        Router,
    };
    use http_body_util::BodyExt;
    use tokio::task::JoinHandle;

    async fn start_server(app: Router) -> (std::net::SocketAddr, JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("failed to bind test server");
        let address = listener.local_addr().expect("test server has no address");
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("test server failed");
        });

        (address, task)
    }

    async fn redirect_once() -> impl IntoResponse {
        (StatusCode::FOUND, [("location", "/final")])
    }

    async fn final_response() -> &'static str {
        "redirect completed"
    }

    async fn redirect_chain(Path(remaining): Path<u8>) -> impl IntoResponse {
        if remaining == 0 {
            (StatusCode::OK, "redirect completed").into_response()
        } else {
            (
                StatusCode::FOUND,
                [("location", format!("/chain/{}", remaining - 1))],
            )
                .into_response()
        }
    }

    #[tokio::test]
    async fn follows_relative_redirect_and_returns_final_response() {
        let app = Router::new()
            .route("/redirect", get(redirect_once))
            .route("/final", get(final_response));
        let (address, server) = start_server(app).await;

        let response = proxy_request(
            Method::GET,
            &format!("http://{address}/redirect"),
            None,
            HeaderMap::new(),
            Body::empty(),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().get("location").is_none());
        let body = response
            .into_body()
            .collect()
            .await
            .expect("response body should be readable")
            .to_bytes();
        assert_eq!(&body[..], b"redirect completed");

        server.abort();
    }

    #[tokio::test]
    async fn returns_bad_gateway_after_more_than_ten_redirects() {
        let app = Router::new().route("/chain/{remaining}", get(redirect_chain));
        let (address, server) = start_server(app).await;

        let response = proxy_request(
            Method::GET,
            &format!("http://{address}/chain/11"),
            None,
            HeaderMap::new(),
            Body::empty(),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_GATEWAY);

        server.abort();
    }
}
