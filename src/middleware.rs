use actix_web::{
    dev::{forward_ready, Service, ServiceRequest, ServiceResponse, Transform},
    Error,
};
use chrono::{DateTime, Utc};
use futures_util::future::LocalBoxFuture;
use std::{
    future::{ready, Ready},
    rc::Rc,
    time::Instant,
};

pub struct RequestLogger;

impl<S, B> Transform<S, ServiceRequest> for RequestLogger
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
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
    B: 'static,
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
            let res = service.call(req).await?;
            
            let duration = start_time.elapsed();
            let duration_ms = duration.as_secs_f64() * 1000.0;
            let status = res.status().as_u16();
            
            // Get response size from Content-Length header if available
            let size_bytes = res
                .headers()
                .get("content-length")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);

            // Format timestamp in ISO 8601 format
            let timestamp: DateTime<Utc> = Utc::now();
            let timestamp_str = timestamp.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string();

            // Format the log message
            let log_message = format!(
                "time={} msg=\"{} {} {} {:.2}ms\" status={} method={} uri={} size_bytes={} duration_ms={:.2} user_agent={} client_ip={}",
                timestamp_str,
                status,
                method,
                uri,
                duration_ms,
                status,
                method,
                uri,
                size_bytes,
                duration_ms,
                user_agent,
                client_ip
            );
            
            // Log in the specified format
            log::info!("{}", log_message);
            
            // Also print to stdout for immediate visibility
            println!("{}", log_message);

            Ok(res)
        })
    }
}
