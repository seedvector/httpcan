use super::*;
use serde_json::Value;

// Simple 1x1 pixel images in different formats
const PNG_IMAGE: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D,
    0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
    0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00,
    0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0x57, 0x63, 0xF8, 0x0F, 0x00, 0x00,
    0x01, 0x00, 0x01, 0x14, 0x6D, 0xD3, 0x8D, 0x00, 0x00, 0x00, 0x00, 0x49,
    0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82
];

const JPEG_IMAGE: &[u8] = &[
    0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
    0x01, 0x01, 0x00, 0x48, 0x00, 0x48, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43,
    0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08, 0x07, 0x07, 0x07, 0x09,
    0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12,
    0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20,
    0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29,
    0x2C, 0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32,
    0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x01,
    0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01,
    0xFF, 0xC4, 0x00, 0x14, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x08, 0xFF, 0xC4,
    0x00, 0x14, 0x10, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xDA, 0x00, 0x0C,
    0x03, 0x01, 0x00, 0x02, 0x11, 0x03, 0x11, 0x00, 0x3F, 0x00, 0x8A, 0x00,
    0xFF, 0xD9
];

const WEBP_IMAGE: &[u8] = &[
    0x52, 0x49, 0x46, 0x46, 0x1A, 0x00, 0x00, 0x00, 0x57, 0x45, 0x42, 0x50,
    0x56, 0x50, 0x38, 0x20, 0x0E, 0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
];

// Parse Accept header to determine preferred image format
fn parse_preferred_format(req: &HttpRequest) -> Option<String> {
    if let Some(accept_header) = req.headers().get("accept") {
        if let Ok(accept_str) = accept_header.to_str() {
            let accept_lower = accept_str.to_lowercase();
            
            // Check in priority order like httpbin: webp → svg → jpeg → png/image/*
            if accept_lower.contains("image/webp") {
                return Some("webp".to_string());
            } else if accept_lower.contains("image/svg+xml") {
                return Some("svg".to_string());
            } else if accept_lower.contains("image/jpeg") {
                return Some("jpeg".to_string());
            } else if accept_lower.contains("image/png") || accept_lower.contains("image/*") {
                return Some("png".to_string());
            }
        }
    }
    None
}

// Check if Accept header should be treated as "no preference" (use httpcan's random behavior)
fn should_use_default(req: &HttpRequest) -> bool {
    if let Some(accept_header) = req.headers().get("accept") {
        if let Ok(accept_str) = accept_header.to_str() {
            let accept_lower = accept_str.to_lowercase();
            // Treat */* as "no preference" - use httpcan's random behavior
            return accept_lower.contains("*/*");
        }
    }
    // No Accept header means use httpcan's random behavior
    true
}

// Load image data from JSON file
fn load_image_data() -> Result<Value, Box<dyn std::error::Error>> {
    let json_str = include_str!("../image_base64.json");
    let data: Value = serde_json::from_str(json_str)?;
    Ok(data)
}

// Get random image data
fn get_random_image() -> Result<(String, String, Vec<u8>), Box<dyn std::error::Error>> {
    let data = load_image_data()?;
    let mut rng = rand::thread_rng();
    
    // Get available formats (jpeg, png, svg, webp)
    let formats: Vec<String> = data.as_object()
        .ok_or("Invalid JSON structure")?
        .keys()
        .cloned()
        .collect();
    
    // Randomly select a format
    let format = formats[rng.gen_range(0..formats.len())].clone();
    let format_data = data[&format].as_object()
        .ok_or("Invalid format data")?;
    
    // Get available colors for this format
    let colors: Vec<String> = format_data.keys().cloned().collect();
    
    // Randomly select a color
    let color = colors[rng.gen_range(0..colors.len())].clone();
    let base64_data = format_data[&color].as_str()
        .ok_or("Invalid base64 data")?;
    
    // Decode base64 data
    let image_data = general_purpose::STANDARD.decode(base64_data)?;
    
    // Determine content type
    let content_type = match format.as_str() {
        "jpeg" => "image/jpeg",
        "png" => "image/png", 
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    };
    
    Ok((content_type.to_string(), color, image_data))
}

// Get random image data for a specific format
fn get_random_image_by_format(format: &str) -> Result<(String, String, Vec<u8>), Box<dyn std::error::Error>> {
    let data = load_image_data()?;
    let mut rng = rand::thread_rng();
    
    let format_data = data[format].as_object()
        .ok_or("Invalid format data")?;
    
    // Get available colors for this format
    let colors: Vec<String> = format_data.keys().cloned().collect();
    
    // Randomly select a color
    let color = colors[rng.gen_range(0..colors.len())].clone();
    let base64_data = format_data[&color].as_str()
        .ok_or("Invalid base64 data")?;
    
    // Decode base64 data
    let image_data = general_purpose::STANDARD.decode(base64_data)?;
    
    // Determine content type
    let content_type = match format {
        "jpeg" => "image/jpeg",
        "png" => "image/png", 
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        _ => "application/octet-stream",
    };
    
    Ok((content_type.to_string(), color, image_data))
}

pub async fn image_handler(req: HttpRequest) -> Result<HttpResponse> {
    // Check if we should use default behavior (no Accept header or */* )
    if should_use_default(&req) {
        // Default behavior: return random format (maintaining httpcan's behavior)
        match get_random_image() {
            Ok((content_type, color, image_data)) => {
                return Ok(HttpResponse::Ok()
                    .content_type(content_type.as_str())
                    .insert_header(("X-Image-Color", color))
                    .insert_header(("X-Image-Format-Source", "random"))
                    .body(image_data));
            }
            Err(_) => {
                // Fallback to default PNG if there's an error
                return Ok(HttpResponse::Ok()
                    .content_type("image/png")
                    .insert_header(("X-Image-Format-Source", "fallback"))
                    .body(PNG_IMAGE));
            }
        }
    }
    
    // Check if Accept header specifies a preferred image format
    if let Some(preferred_format) = parse_preferred_format(&req) {
        // Try to get image in the preferred format
        match get_random_image_by_format(&preferred_format) {
            Ok((content_type, color, image_data)) => {
                return Ok(HttpResponse::Ok()
                    .content_type(content_type.as_str())
                    .insert_header(("X-Image-Color", color))
                    .insert_header(("X-Image-Format-Source", "accept-header"))
                    .body(image_data));
            }
            Err(_) => {
                // If preferred format fails, fall through to 406
            }
        }
    }
    
    // Accept header present but no supported image format found
    Ok(HttpResponse::NotAcceptable().json(json!({
        "error": "Unsupported media type"
    })))
}

pub async fn image_png_handler(_req: HttpRequest) -> Result<HttpResponse> {
    match get_random_image_by_format("png") {
        Ok((content_type, color, image_data)) => {
            Ok(HttpResponse::Ok()
                .content_type(content_type.as_str())
                .insert_header(("X-Image-Color", color))
                .body(image_data))
        }
        Err(_) => {
            // Fallback to default PNG if there's an error
            Ok(HttpResponse::Ok()
                .content_type("image/png")
                .body(PNG_IMAGE))
        }
    }
}

pub async fn image_jpeg_handler(_req: HttpRequest) -> Result<HttpResponse> {
    match get_random_image_by_format("jpeg") {
        Ok((content_type, color, image_data)) => {
            Ok(HttpResponse::Ok()
                .content_type(content_type.as_str())
                .insert_header(("X-Image-Color", color))
                .body(image_data))
        }
        Err(_) => {
            // Fallback to default JPEG if there's an error
            Ok(HttpResponse::Ok()
                .content_type("image/jpeg")
                .body(JPEG_IMAGE))
        }
    }
}

pub async fn image_webp_handler(_req: HttpRequest) -> Result<HttpResponse> {
    match get_random_image_by_format("webp") {
        Ok((content_type, color, image_data)) => {
            Ok(HttpResponse::Ok()
                .content_type(content_type.as_str())
                .insert_header(("X-Image-Color", color))
                .body(image_data))
        }
        Err(_) => {
            // Fallback to default WebP if there's an error
            Ok(HttpResponse::Ok()
                .content_type("image/webp")
                .body(WEBP_IMAGE))
        }
    }
}

pub async fn image_svg_handler(_req: HttpRequest) -> Result<HttpResponse> {
    match get_random_image_by_format("svg") {
        Ok((content_type, color, image_data)) => {
            Ok(HttpResponse::Ok()
                .content_type(content_type.as_str())
                .insert_header(("X-Image-Color", color))
                .body(image_data))
        }
        Err(_) => {
            // Fallback to default SVG if there's an error
            let svg_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<svg width="100" height="100" xmlns="http://www.w3.org/2000/svg">
  <rect width="100" height="100" fill="red"/>
  <circle cx="50" cy="50" r="40" fill="blue"/>
  <text x="50" y="55" font-family="Arial" font-size="12" text-anchor="middle" fill="white">SVG</text>
</svg>"#;
            Ok(HttpResponse::Ok()
                .content_type("image/svg+xml")
                .body(svg_content))
        }
    }
}
