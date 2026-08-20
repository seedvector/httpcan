use actix_web::{
    body::{BodySize, MessageBody},
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    http::header::{HeaderName, HeaderValue},
    Error,
};
use chrono::{DateTime, Utc};
use futures_util::future::LocalBoxFuture;
use std::{
    future::{ready, Ready},
    rc::Rc,
    sync::LazyLock,
    time::Instant,
};

/// Static response headers applied to every response: the httpcan version
/// (httpbin #431) and any `XHTTPCAN_*` env vars mapped to response headers
/// (httpbin #565). Computed once and cached.
fn static_response_headers() -> &'static [(HeaderName, HeaderValue)] {
    static HEADERS: LazyLock<Vec<(HeaderName, HeaderValue)>> = LazyLock::new(|| {
        let mut headers = Vec::new();
        if let Ok(v) = HeaderValue::from_str(env!("CARGO_PKG_VERSION")) {
            headers.push((HeaderName::from_static("x-httpcan-version"), v));
        }
        for (key, value) in std::env::vars() {
            if let Some(rest) = key.strip_prefix("XHTTPCAN_") {
                let name = rest.replace('_', "-");
                if let (Ok(n), Ok(v)) = (
                    HeaderName::from_bytes(name.as_bytes()),
                    HeaderValue::from_str(&value),
                ) {
                    headers.push((n, v));
                }
            }
        }
        headers
    });
    &HEADERS
}

/// Response body size in bytes for the access log; streaming and
/// unknown-size bodies report 0. The Content-Length header cannot be used:
/// actix-http's encoder sets it only while writing to the socket, so a
/// header read at middleware time always returned 0.
pub(crate) fn response_size_bytes<B: MessageBody>(res: &ServiceResponse<B>) -> u64 {
    match res.response().body().size() {
        BodySize::Sized(n) => n as u64,
        _ => 0,
    }
}
pub struct RequestLogger;

impl<S, B> Transform<S, ServiceRequest> for RequestLogger
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static + actix_web::body::MessageBody,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type InitError = ();
    type Transform = RequestLoggerMiddleware<S>;
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(RequestLoggerMiddleware {
            service: Rc::new(service),
        }))
    }
}

pub struct RequestLoggerMiddleware<S> {
    service: Rc<S>,
}
impl<S, B> Service<ServiceRequest> for RequestLoggerMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<B>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    forward_ready!(service);

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let start_time = Instant::now();
        let method = req.method().to_string();
        let uri = req.uri().to_string();
        let user_agent = req
            .headers()
            .get("user-agent")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("-")
            .to_string();

        // Get client IP from connection info or X-Forwarded-For header
        let client_ip = req
            .connection_info()
            .realip_remote_addr()
            .unwrap_or("unknown")
            .to_string();

        let service = self.service.clone();

        Box::pin(async move {
            let mut res = service.call(req).await?;

            let duration = start_time.elapsed();
            let duration_ms = duration.as_secs_f64() * 1000.0;
            let status = res.status().as_u16();

            // Response size from the body's known size. The Content-Length
            // header cannot be used here: actix-http's encoder computes it
            // only while writing to the socket, so the header read always
            // returned 0.
            let size_bytes = response_size_bytes(&res);

            // Format timestamp in ISO 8601 format
            let timestamp: DateTime<Utc> = Utc::now();
            let timestamp_str = timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

            // Format the log message in the specified format
            let log_message = format!(
                "time={} level=info method={} uri={} status={} duration_ms={:.3} size_bytes={} client_ip={} user_agent=\"{}\"",
                timestamp_str,
                method,
                uri,
                status,
                duration_ms,
                size_bytes,
                client_ip,
                user_agent
            );

            // Single log channel: the `log` facade (async stderr writer,
            // `crate::logging`; never blocks workers). The default filter
            // is info (see main.rs), so request logs are visible out of
            // the box and fully controllable via RUST_LOG.
            log::info!("{}", log_message);

            // Observability response headers (httpbin #431/#560/#565).
            let headers = res.headers_mut();
            for (name, value) in static_response_headers() {
                headers.insert(name.clone(), value.clone());
            }
            if let Ok(dur) = HeaderValue::from_str(&format!("app;dur={duration_ms:.3}")) {
                headers.insert(HeaderName::from_static("server-timing"), dur);
            }

            Ok(res)
        })
    }
}
