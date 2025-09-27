use super::*;
use actix_web::{web::Bytes, HttpResponseBuilder, body::SizedStream};
use rand::{SeedableRng, Rng};
use rand::rngs::StdRng;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Deserialize)]
pub struct DripQuery {
    duration: Option<f64>,
    numbytes: Option<usize>,
    code: Option<u16>,
    delay: Option<f64>,
}

#[derive(Deserialize)]
pub struct BytesQuery {
    seed: Option<u64>,
}

#[derive(Deserialize)]
pub struct StreamBytesQuery {
    seed: Option<u64>,
    chunk_size: Option<usize>,
}

#[derive(Deserialize)]
pub struct RangeQuery {
    chunk_size: Option<usize>,
    duration: Option<f64>,
}

// Helper function to parse Range header - mimics httpbin's __parse_request_range
fn parse_request_range(range_header_value: &str) -> (Option<usize>, Option<usize>) {
    if !range_header_value.starts_with("bytes=") {
        return (None, None);
    }
    
    let range_part = &range_header_value[6..];
    
    // Handle multiple ranges (take first one like httpbin)
    let first_range = range_part.split(',').next().unwrap_or(range_part);
    
    if let Some((start_str, end_str)) = first_range.split_once('-') {
        let first_byte_pos = if start_str.is_empty() {
            None
        } else {
            start_str.parse().ok()
        };
        
        let last_byte_pos = if end_str.is_empty() {
            None
        } else {
            end_str.parse().ok()
        };
        
        return (first_byte_pos, last_byte_pos);
    }
    
    (None, None)
}

// Helper function to get request range - mimics httpbin's get_request_range
fn get_request_range(req: &HttpRequest, upper_bound: usize) -> (usize, usize) {
    let (first_byte_pos, last_byte_pos) = if let Some(range_header) = req.headers().get("Range") {
        if let Ok(range_str) = range_header.to_str() {
            parse_request_range(range_str)
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };
    
    let (first_byte_pos, last_byte_pos) = match (first_byte_pos, last_byte_pos) {
        (None, None) => {
            // Request full range
            (0, upper_bound - 1)
        }
        (None, Some(suffix_length)) => {
            // Request the last X bytes (suffix-byte-range-spec: "-500")
            let first_pos = if upper_bound >= suffix_length {
                upper_bound - suffix_length
            } else {
                0
            };
            (first_pos, upper_bound - 1)
        }
        (Some(start), None) => {
            // Request from X to end
            (start, upper_bound - 1)
        }
        (Some(start), Some(end)) => {
            // Request specific range
            (start, end)
        }
    };
    
    (first_byte_pos, last_byte_pos)
}

pub async fn uuid_handler(_req: HttpRequest) -> Result<HttpResponse> {
    let uuid = Uuid::new_v4();
    Ok(HttpResponse::Ok().json(json!({
        "uuid": uuid.to_string()
    })))
}

pub async fn base64_handler(
    _req: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let encoded_value = path.into_inner();
    
    match general_purpose::STANDARD.decode(&encoded_value) {
        Ok(decoded_bytes) => {
            match String::from_utf8(decoded_bytes) {
                Ok(decoded_string) => {
                    Ok(HttpResponse::Ok()
                        .content_type("text/plain")
                        .body(decoded_string))
                }
                Err(_) => {
                    Ok(HttpResponse::BadRequest()
                        .json(json!({
                            "error": "Invalid UTF-8 in decoded data"
                        })))
                }
            }
        }
        Err(_) => {
            Ok(HttpResponse::BadRequest()
                .json(json!({
                    "error": "Invalid base64 data"
                })))
        }
    }
}

pub async fn bytes_handler(
    _req: HttpRequest,
    path: web::Path<usize>,
    query: web::Query<BytesQuery>,
) -> Result<HttpResponse> {
    let n = path.into_inner();
    let n = n.min(100 * 1024); // Limit to 100KB
    
    // Generate random bytes using the same method as httpbin
    let random_bytes: Vec<u8> = if let Some(seed) = query.seed {
        let mut rng = StdRng::seed_from_u64(seed);
        (0..n).map(|_| rng.gen_range(0..=255)).collect()
    } else {
        let mut rng = rand::thread_rng();
        (0..n).map(|_| rng.gen_range(0..=255)).collect()
    };
    
    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .body(random_bytes))
}

pub async fn stream_bytes_handler(
    _req: HttpRequest,
    path: web::Path<usize>,
    query: web::Query<StreamBytesQuery>,
) -> Result<HttpResponse> {
    let n = path.into_inner();
    let n = n.min(100 * 1024); // Limit to 100KB
    
    // Parse chunk_size parameter
    let chunk_size = query.chunk_size.unwrap_or(10 * 1024).max(1);
    
    // Initialize RNG with seed if provided
    let seed = query.seed;
    
    let stream = async_stream::stream! {
        let mut chunks = Vec::new();
        
        if let Some(seed) = seed {
            let mut rng = StdRng::seed_from_u64(seed);
            for _ in 0..n {
                chunks.push(rng.gen_range(0..=255));
                
                if chunks.len() == chunk_size {
                    yield Ok::<_, actix_web::Error>(Bytes::from(chunks.clone()));
                    chunks.clear();
                }
            }
        } else {
            let mut rng = rand::thread_rng();
            for _ in 0..n {
                chunks.push(rng.gen_range(0..=255));
                
                if chunks.len() == chunk_size {
                    yield Ok::<_, actix_web::Error>(Bytes::from(chunks.clone()));
                    chunks.clear();
                }
            }
        }
        
        // Yield remaining bytes if any
        if !chunks.is_empty() {
            yield Ok::<_, actix_web::Error>(Bytes::from(chunks));
        }
    };
    
    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .streaming(stream))
}

pub async fn stream_handler(
    req: HttpRequest,
    path: web::Path<usize>,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let n = path.into_inner();
    let n = n.min(100); // Limit to 100 lines
    
    // Extract request information similar to httpbin's get_dict("url", "args", "headers", "origin")
    let base_response = extract_get_request_info(&req, &config.exclude_headers);
    
    let stream = futures_util::stream::iter((0..n).map(move |i| {
        let response = json!({
            "url": base_response.url,
            "args": base_response.args,
            "headers": base_response.headers,
            "origin": base_response.origin,
            "id": i
        });
        
        // Format as JSON string with newline, matching httpbin's json.dumps(response) + "\n"
        Ok::<_, actix_web::Error>(Bytes::from(format!("{}\n", response)))
    }));
    
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .streaming(stream))
}

pub async fn range_handler(
    req: HttpRequest,
    path: web::Path<usize>,
    query: web::Query<RangeQuery>,
) -> Result<HttpResponse> {
    let numbytes = path.into_inner();
    
    // Check bounds first like httpbin
    if numbytes == 0 || numbytes > (100 * 1024) {
        return Ok(HttpResponse::NotFound()
            .append_header(("ETag", format!("range{}", numbytes)))
            .append_header(("Accept-Ranges", "bytes"))
            .body("number of bytes must be in the range (0, 102400]"));
    }
    
    let chunk_size = query.chunk_size.unwrap_or(10 * 1024).max(1);
    let duration = query.duration.unwrap_or(0.0);
    
    // Extract range information from headers using httpbin-compatible logic
    let (first_byte_pos, last_byte_pos) = get_request_range(&req, numbytes);
    
    // Validate range like httpbin
    if first_byte_pos > last_byte_pos 
        || first_byte_pos >= numbytes
        || last_byte_pos >= numbytes {
        return Ok(HttpResponse::RangeNotSatisfiable()
            .append_header(("ETag", format!("range{}", numbytes)))
            .append_header(("Accept-Ranges", "bytes"))
            .append_header(("Content-Range", format!("bytes */{}", numbytes)))
            .append_header(("Content-Length", "0"))
            .finish());
    }
    
    let range_length = (last_byte_pos + 1) - first_byte_pos;
    let pause_per_byte = if range_length > 0 { 
        Duration::from_secs_f64(duration / range_length as f64) 
    } else { 
        Duration::ZERO 
    };
    
    // Create streaming response
    let stream = async_stream::stream! {
        let mut chunks = Vec::new();
        
        for i in first_byte_pos..=last_byte_pos {
            // Use deterministic data generation like httpbin: ord("a") + (i % 26)
            // This ensures consistent data for the same byte position across requests
            chunks.push(b'a' + (i % 26) as u8);
            
            if chunks.len() == chunk_size || i == last_byte_pos {
                yield Ok::<_, actix_web::Error>(Bytes::from(chunks.clone()));
                
                // Apply timing delay if specified
                if pause_per_byte.as_nanos() > 0 && chunks.len() > 0 {
                    tokio::time::sleep(pause_per_byte * chunks.len() as u32).await;
                }
                
                chunks.clear();
            }
        }
    };
    
    let content_range = format!("bytes {}-{}/{}", first_byte_pos, last_byte_pos, numbytes);
    
    // Determine status code: 200 for full content, 206 for partial content
    let is_full_content = first_byte_pos == 0 && last_byte_pos == (numbytes - 1);
    let status = if is_full_content { 
        StatusCode::OK 
    } else { 
        StatusCode::PARTIAL_CONTENT 
    };
    
    Ok(HttpResponseBuilder::new(status)
        .content_type("application/octet-stream")
        .append_header(("ETag", format!("range{}", numbytes)))
        .append_header(("Accept-Ranges", "bytes"))
        .append_header(("Content-Length", range_length.to_string()))
        .append_header(("Content-Range", content_range))
        .body(SizedStream::new(range_length as u64, Box::pin(stream))))
}

pub async fn links_handler(
    _req: HttpRequest,
    path: web::Path<(usize, usize)>,
) -> Result<HttpResponse> {
    let (n, offset) = path.into_inner();
    let n = n.max(1).min(200); // Limit to between 1 and 200 links
    
    let mut html = String::from("<!DOCTYPE html><html><head><title>Links</title></head><body>");
    
    for i in 0..n {
        if i == offset {
            html.push_str(&format!("{}<br>", i));
        } else {
            html.push_str(&format!("<a href='/links/{}/{}'>{}</a><br>", n, i, i));
        }
    }
    
    html.push_str("</body></html>");
    
    Ok(HttpResponse::Ok()
        .content_type("text/html")
        .body(html))
}

pub async fn links_redirect_handler(
    _req: HttpRequest,
    path: web::Path<usize>,
) -> Result<HttpResponse> {
    let n = path.into_inner();
    Ok(HttpResponse::Found()
        .append_header(("Location", format!("/links/{}/0", n)))
        .body(""))
}

pub async fn drip_handler(
    _req: HttpRequest,
    query: web::Query<DripQuery>,
) -> Result<HttpResponse> {
    let duration = query.duration.unwrap_or(2.0);
    let numbytes = query.numbytes.unwrap_or(10);
    let code = query.code.unwrap_or(200);
    let delay = query.delay.unwrap_or(0.0);
    
    // Validate parameters
    if numbytes == 0 {
        return Ok(HttpResponse::BadRequest()
            .json(json!({
                "error": "number of bytes must be positive"
            })));
    }
    
    // Set reasonable limit (10MB)
    let numbytes = numbytes.min(10 * 1024 * 1024);
    
    // Validate status code
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::OK);
    
    // Initial delay
    if delay > 0.0 {
        sleep(Duration::from_secs_f64(delay)).await;
    }
    
    // Calculate pause between bytes
    let pause = if numbytes == 1 {
        Duration::from_secs_f64(duration)
    } else {
        Duration::from_secs_f64(duration / (numbytes as f64 - 1.0))
    };
    
    // Create streaming response using SizedStream
    let stream = async_stream::stream! {
        for i in 0..numbytes {
            // Yield a single '*' byte
            yield Ok::<_, actix_web::Error>(Bytes::from("*"));
            
            // Don't pause after the last byte
            if i < numbytes - 1 && pause.as_nanos() > 0 {
                sleep(pause).await;
            }
        }
    };
    
    // Use SizedStream to set both content length and streaming
    Ok(HttpResponseBuilder::new(status)
        .content_type("application/octet-stream")
        .body(SizedStream::new(numbytes as u64, Box::pin(stream))))
}

pub async fn delay_handler_get(
    req: HttpRequest,
    path: web::Path<u64>,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let delay_seconds = path.into_inner().min(10); // Max 10 seconds
    
    sleep(Duration::from_secs(delay_seconds)).await;
    
    let mut request_info = extract_request_info(&req, None, &config.exclude_headers);
    fix_request_info_url(&req, &mut request_info);
    Ok(HttpResponse::Ok().json(request_info))
}

pub async fn delay_handler(
    req: HttpRequest,
    path: web::Path<u64>,
    body: String,
    config: web::Data<AppConfig>,
) -> Result<HttpResponse> {
    let delay_seconds = path.into_inner().min(10); // Max 10 seconds
    
    sleep(Duration::from_secs(delay_seconds)).await;
    
    let mut request_info = extract_request_info(&req, Some(&body), &config.exclude_headers);
    fix_request_info_url(&req, &mut request_info);
    Ok(HttpResponse::Ok().json(request_info))
}
