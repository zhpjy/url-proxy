use axum::{
    body::Body,
    http::{HeaderMap, Method, StatusCode, },
    response::{IntoResponse, Response},
};
use http_body_util::BodyStream;
use futures_util::TryStreamExt;
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

    // Convert the axum Body into a stream of bytes for reqwest.
    // BodyStream yields Result<Frame<Bytes>, Error>.
    let body_stream = BodyStream::new(body);
    // We need to filter out trailer frames and extract bytes from data frames.
    let stream_of_bytes = body_stream.try_filter_map(|frame| async move {
        Ok(frame.into_data().ok())
    });
    // Map the error type to what reqwest::Body::wrap_stream expects.
    let stream = stream_of_bytes.map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()));

    let reqwest_body = reqwest::Body::wrap_stream(stream);

    // Build the request
    let mut request_builder = client.request(method, &target_url).body(reqwest_body);

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
                    (StatusCode::INTERNAL_SERVER_ERROR, "Failed to construct response").into_response()
                }
            }
        }
        Err(e) => {
            error!("Proxy request failed: {}", e);
            (StatusCode::BAD_GATEWAY, "Proxy request failed").into_response()
        }
    }
}