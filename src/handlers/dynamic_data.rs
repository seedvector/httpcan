use super::*;
use actix_web::{web::Bytes, HttpResponseBuilder};

#[derive(Deserialize)]
pub struct DripQuery {
    duration: Option<f64>,
    numbytes: Option<usize>,
    code: Option<u16>,
    delay: Option<f64>,
}

pub async fn uuid_handler(req: HttpRequest) -> Result<HttpResponse> {
    let uuid = Uuid::new_v4();
    Ok(HttpResponse::Ok().json(json!({
        "uuid": uuid.to_string()
    })))
}

pub async fn base64_handler(
    req: HttpRequest,
    path: web::Path<String>,
) -> Result<HttpResponse> {
    let encoded_value = path.into_inner();
    
    match general_purpose::STANDARD.decode(&encoded_value) {
        Ok(decoded_bytes) => {
            match String::from_utf8(decoded_bytes) {
                Ok(decoded_string) => {
                    Ok(HttpResponse::Ok()
                        .content_type("text/html")
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
                    "error": "Invalid base64 encoding"
                })))
        }
    }
}

pub async fn bytes_handler(
    req: HttpRequest,
    path: web::Path<usize>,
) -> Result<HttpResponse> {
    let n = path.into_inner();
    let n = n.min(102400); // Limit to 100KB
    
    let mut rng = rand::thread_rng();
    let random_bytes: Vec<u8> = (0..n).map(|_| rng.gen()).collect();
    
    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .body(random_bytes))
}

pub async fn stream_bytes_handler(
    req: HttpRequest,
    path: web::Path<usize>,
) -> Result<HttpResponse> {
    let n = path.into_inner();
    let n = n.min(102400); // Limit to 100KB
    
    let mut rng = rand::thread_rng();
    let random_bytes: Vec<u8> = (0..n).map(|_| rng.gen()).collect();
    
    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .streaming(futures_util::stream::once(async move {
            Ok::<_, actix_web::Error>(Bytes::from(random_bytes))
        })))
}

pub async fn stream_handler(
    req: HttpRequest,
    path: web::Path<usize>,
) -> Result<HttpResponse> {
    let n = path.into_inner();
    let n = n.min(100); // Limit to 100 lines
    
    let stream = futures_util::stream::iter((0..n).map(move |i| {
        let data = json!({
            "url": format!("https://httpbin.org/stream/{}", n),
            "args": {},
            "headers": {
                "Host": "httpbin.org"
            },
            "origin": "127.0.0.1",
            "id": i
        });
        Ok::<_, actix_web::Error>(Bytes::from(format!("{}\n", data)))
    }));
    
    Ok(HttpResponse::Ok()
        .content_type("application/json")
        .streaming(stream))
}

pub async fn range_handler(
    req: HttpRequest,
    path: web::Path<usize>,
) -> Result<HttpResponse> {
    let numbytes = path.into_inner();
    let numbytes = numbytes.min(102400); // Limit to 100KB
    
    let mut rng = rand::thread_rng();
    let random_bytes: Vec<u8> = (0..numbytes).map(|_| rng.gen()).collect();
    
    // Check for Range header
    if let Some(range_header) = req.headers().get("Range") {
        if let Ok(range_str) = range_header.to_str() {
            if range_str.starts_with("bytes=") {
                let range_part = &range_str[6..];
                if let Some((start_str, end_str)) = range_part.split_once('-') {
                    let start: usize = start_str.parse().unwrap_or(0);
                    let end: usize = if end_str.is_empty() {
                        numbytes - 1
                    } else {
                        end_str.parse().unwrap_or(numbytes - 1).min(numbytes - 1)
                    };
                    
                    if start <= end && start < numbytes {
                        let slice = &random_bytes[start..=end];
                        return Ok(HttpResponse::PartialContent()
                            .content_type("application/octet-stream")
                            .append_header(("Content-Range", format!("bytes {}-{}/{}", start, end, numbytes)))
                            .append_header(("Accept-Ranges", "bytes"))
                            .body(slice.to_vec()));
                    }
                }
            }
        }
    }
    
    Ok(HttpResponse::Ok()
        .content_type("application/octet-stream")
        .append_header(("Accept-Ranges", "bytes"))
        .body(random_bytes))
}

pub async fn links_handler(
    req: HttpRequest,
    path: web::Path<(usize, usize)>,
) -> Result<HttpResponse> {
    let (n, offset) = path.into_inner();
    let n = n.min(200); // Limit to 200 links
    
    let mut html = String::from("<!DOCTYPE html><html><head><title>Links</title></head><body>");
    
    for i in 0..n {
        let link_num = offset + i;
        html.push_str(&format!(
            "<a href=\"/links/{}/{}\">{}</a><br>",
            n, link_num, link_num
        ));
    }
    
    html.push_str("</body></html>");
    
    Ok(HttpResponse::Ok()
        .content_type("text/html")
        .body(html))
}

pub async fn drip_handler(
    req: HttpRequest,
    query: web::Query<DripQuery>,
) -> Result<HttpResponse> {
    let duration = query.duration.unwrap_or(2.0);
    let numbytes = query.numbytes.unwrap_or(10);
    let code = query.code.unwrap_or(200);
    let delay = query.delay.unwrap_or(2.0);
    
    // Initial delay
    sleep(Duration::from_secs_f64(delay)).await;
    
    let mut rng = rand::thread_rng();
    let random_bytes: Vec<u8> = (0..numbytes).map(|_| rng.gen()).collect();
    
    let status = StatusCode::from_u16(code).unwrap_or(StatusCode::OK);
    
    // For simplicity, we'll return all data at once after the delay
    // In a full implementation, this would drip data over time
    Ok(HttpResponseBuilder::new(status)
        .content_type("application/octet-stream")
        .body(random_bytes))
}

pub async fn delay_handler(
    req: HttpRequest,
    path: web::Path<u64>,
    body: String,
) -> Result<HttpResponse> {
    let delay_seconds = path.into_inner().min(10); // Max 10 seconds
    
    sleep(Duration::from_secs(delay_seconds)).await;
    
    let request_info = extract_request_info(&req, Some(&body));
    Ok(HttpResponse::Ok().json(request_info))
}
