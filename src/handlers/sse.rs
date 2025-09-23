use actix_web::{web, HttpRequest, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;
use chrono::Utc;

#[derive(Deserialize)]
pub struct SseParams {
    /// Number of events to send (default: 5)
    count: Option<u32>,
    /// Delay between events in milliseconds (default: 1000)
    delay: Option<u64>,
    /// Message format: "simple", "openai", "custom" (default: "simple")
    format: Option<String>,
    /// Custom message content for "custom" format
    message: Option<String>,
    /// Event type for SSE events (default: "message")
    event_type: Option<String>,
}

#[derive(Serialize)]
pub struct OpenAIChoice {
    pub delta: OpenAIDelta,
    pub index: u32,
    pub finish_reason: Option<String>,
}

#[derive(Serialize)]
pub struct OpenAIDelta {
    pub content: Option<String>,
    pub role: Option<String>,
}

#[derive(Serialize)]
pub struct OpenAIMessage {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<OpenAIChoice>,
}

#[derive(Serialize)]
pub struct OllamaResponse {
    pub model: String,
    pub created_at: String,
    pub response: String,
    pub done: bool,
    pub context: Option<Vec<u32>>,
    pub total_duration: Option<u64>,
    pub load_duration: Option<u64>,
    pub prompt_eval_count: Option<u32>,
    pub prompt_eval_duration: Option<u64>,
    pub eval_count: Option<u32>,
    pub eval_duration: Option<u64>,
}

#[derive(Deserialize)]
pub struct NdjsonParams {
    /// Number of events to send (default: 5)
    count: Option<u32>,
    /// Delay between events in milliseconds (default: 1000)
    delay: Option<u64>,
    /// Message format: "simple", "openai", "ollama", "custom" (default: "simple")
    format: Option<String>,
    /// Custom message content for "custom" format
    message: Option<String>,
    /// Model name for ollama format (default: "llama2")
    model: Option<String>,
}

/// SSE endpoint that streams events with configurable count and format
/// 
/// Query parameters:
/// - count: number of events to send (default: 5, max: 100)
/// - delay: delay between events in milliseconds (default: 1000, max: 10000)
/// - format: message format - "simple", "openai", "custom" (default: "simple")
/// - message: custom message content (used with format=custom)
/// - event_type: SSE event type (default: "message")
pub async fn sse_handler(_req: HttpRequest, query: web::Query<SseParams>) -> Result<HttpResponse> {
    let count = query.count.unwrap_or(5).min(100); // Max 100 events
    let delay = query.delay.unwrap_or(1000).min(10000); // Max 10 seconds delay
    let format = query.format.as_deref().unwrap_or("simple").to_string();
    let event_type = query.event_type.as_deref().unwrap_or("message").to_string();
    let custom_message = query.message.as_deref().unwrap_or("Hello from HTTPCan SSE!").to_string();

    let stream = async_stream::stream! {
        for i in 0..count {
            let event_data = match format.as_str() {
                "openai" => generate_openai_event(i, count),
                "custom" => custom_message.clone(),
                _ => {
                    // Simple format - consistent with NDJSON
                    let simple_obj = serde_json::json!({
                        "event": i + 1,
                        "message": format!("Hello from HTTPCan SSE! Event {}/{}", i + 1, count),
                        "timestamp": Utc::now().to_rfc3339()
                    });
                    serde_json::to_string(&simple_obj).unwrap()
                }
            };

            // Format as SSE event
            let sse_event = if event_type == "message" && format != "openai" {
                format!("data: {}\n\n", event_data)
            } else {
                format!("event: {}\ndata: {}\n\n", event_type, event_data)
            };

            yield Ok::<actix_web::web::Bytes, actix_web::Error>(actix_web::web::Bytes::from(sse_event));

            // Don't delay after the last event
            if i < count - 1 {
                sleep(Duration::from_millis(delay)).await;
            }
        }

        // Send final event to indicate completion
        let final_event = match format.as_str() {
            "openai" => {
                let final_chunk = OpenAIMessage {
                    id: format!("chatcmpl-{}", Uuid::new_v4().simple()),
                    object: "chat.completion.chunk".to_string(),
                    created: Utc::now().timestamp(),
                    model: "httpcan-sse".to_string(),
                    choices: vec![OpenAIChoice {
                        delta: OpenAIDelta {
                            content: None,
                            role: None,
                        },
                        index: 0,
                        finish_reason: Some("stop".to_string()),
                    }],
                };
                format!("data: {}\n\n", serde_json::to_string(&final_chunk).unwrap())
            },
            _ => "event: end\ndata: Stream completed\n\n".to_string()
        };

        yield Ok::<actix_web::web::Bytes, actix_web::Error>(actix_web::web::Bytes::from(final_event));

        // Send the SSE termination
        yield Ok::<actix_web::web::Bytes, actix_web::Error>(actix_web::web::Bytes::from("data: [DONE]\n\n"));
    };

    Ok(HttpResponse::Ok()
        .content_type("text/event-stream")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .insert_header(("Access-Control-Allow-Origin", "*"))
        .insert_header(("Access-Control-Allow-Headers", "Cache-Control"))
        .streaming(stream))
}

fn generate_openai_event(index: u32, total: u32) -> String {
    let content = if index < 16 {
        let messages = [
            "Hello", " from", " HTTPCan", " SSE", " endpoint!", " This", " is", " streaming",
            " like", " OpenAI", " ChatGPT", " API.", " Event", "", " of", ""
        ];
        
        if index == 13 {
            Some(format!(" {}", index + 1))
        } else if index == 15 {
            Some(format!(" {}.", total))
        } else {
            Some(messages[index as usize].to_string())
        }
    } else if index == 0 {
        Some("Hello from HTTPCan SSE endpoint!".to_string())
    } else {
        Some(format!(" Chunk {}", index + 1))
    };

    let chunk = OpenAIMessage {
        id: format!("chatcmpl-{}", Uuid::new_v4().simple()),
        object: "chat.completion.chunk".to_string(),
        created: Utc::now().timestamp(),
        model: "httpcan-sse".to_string(),
        choices: vec![OpenAIChoice {
            delta: OpenAIDelta {
                content,
                role: if index == 0 { Some("assistant".to_string()) } else { None },
            },
            index: 0,
            finish_reason: None,
        }],
    };

    serde_json::to_string(&chunk).unwrap()
}

fn generate_ollama_event(index: u32, total: u32, model: &str) -> String {
    let responses = [
        "Hello", " from", " HTTPCan", " NDJSON", " endpoint!", " This", " is", " streaming",
        " like", " Ollama", " API.", " Response", "", " of", ""
    ];
    
    let response_text = if index < responses.len() as u32 {
        if index == 12 {
            format!(" {}", index + 1)
        } else if index == 14 {
            format!(" {}.", total)
        } else {
            responses[index as usize].to_string()
        }
    } else {
        format!(" Token {}", index + 1)
    };

    let is_done = index >= total - 1;
    
    let ollama_response = OllamaResponse {
        model: model.to_string(),
        created_at: Utc::now().to_rfc3339(),
        response: response_text,
        done: is_done,
        context: if is_done { Some(vec![1, 2, 3, 4, 5]) } else { None },
        total_duration: if is_done { Some(1234567890) } else { None },
        load_duration: if is_done { Some(123456) } else { None },
        prompt_eval_count: if is_done { Some(10) } else { None },
        prompt_eval_duration: if is_done { Some(987654) } else { None },
        eval_count: if is_done { Some(index + 1) } else { None },
        eval_duration: if is_done { Some(5432109) } else { None },
    };

    serde_json::to_string(&ollama_response).unwrap()
}

/// NDJSON endpoint that streams JSON objects separated by newlines
/// 
/// Query parameters:
/// - count: number of events to send (default: 5, max: 100)
/// - delay: delay between events in milliseconds (default: 1000, max: 10000)
/// - format: message format - "simple", "openai", "ollama", "custom" (default: "simple")
/// - message: custom message content (used with format=custom)
/// - model: model name for ollama format (default: "llama2")
pub async fn ndjson_handler(_req: HttpRequest, query: web::Query<NdjsonParams>) -> Result<HttpResponse> {
    let count = query.count.unwrap_or(5).min(100); // Max 100 events
    let delay = query.delay.unwrap_or(1000).min(10000); // Max 10 seconds delay
    let format = query.format.as_deref().unwrap_or("simple").to_string();
    let custom_message = query.message.as_deref().unwrap_or("Hello from HTTPCan NDJSON!").to_string();
    let model = query.model.as_deref().unwrap_or("llama2").to_string();

    let stream = async_stream::stream! {
        for i in 0..count {
            let event_data = match format.as_str() {
                "openai" => generate_openai_event(i, count),
                "ollama" => generate_ollama_event(i, count, &model),
                "custom" => {
                    let custom_obj = serde_json::json!({
                        "message": custom_message,
                        "index": i + 1,
                        "total": count,
                        "timestamp": Utc::now().to_rfc3339()
                    });
                    serde_json::to_string(&custom_obj).unwrap()
                },
                _ => {
                    // Simple format
                    let simple_obj = serde_json::json!({
                        "event": i + 1,
                        "message": format!("Hello from HTTPCan NDJSON! Event {}/{}", i + 1, count),
                        "timestamp": Utc::now().to_rfc3339()
                    });
                    serde_json::to_string(&simple_obj).unwrap()
                }
            };

            // Format as NDJSON (JSON object followed by newline)
            let ndjson_line = format!("{}\n", event_data);
            yield Ok::<actix_web::web::Bytes, actix_web::Error>(actix_web::web::Bytes::from(ndjson_line));

            // Don't delay after the last event
            if i < count - 1 {
                sleep(Duration::from_millis(delay)).await;
            }
        }
    };

    Ok(HttpResponse::Ok()
        .content_type("application/x-ndjson")
        .insert_header(("Cache-Control", "no-cache"))
        .insert_header(("Connection", "keep-alive"))
        .insert_header(("Access-Control-Allow-Origin", "*"))
        .insert_header(("Access-Control-Allow-Headers", "Cache-Control"))
        .streaming(stream))
}

/// NDJSON endpoint with path parameters for convenience
/// GET /ndjson/{count} - sends {count} events with default settings
/// GET /ndjson/{count}/{delay} - sends {count} events with {delay}ms between events
pub async fn ndjson_path_handler(
    req: HttpRequest, 
    path: web::Path<(u32,)>,
    query: web::Query<NdjsonParams>
) -> Result<HttpResponse> {
    let (count,) = path.into_inner();
    
    let params = NdjsonParams {
        count: Some(count.min(100)),
        delay: query.delay,
        format: query.format.clone(),
        message: query.message.clone(),
        model: query.model.clone(),
    };

    ndjson_handler(req, web::Query(params)).await
}

/// NDJSON endpoint with both count and delay as path parameters
pub async fn ndjson_path_with_delay_handler(
    req: HttpRequest, 
    path: web::Path<(u32, u64)>,
    query: web::Query<NdjsonParams>
) -> Result<HttpResponse> {
    let (count, delay) = path.into_inner();
    
    let params = NdjsonParams {
        count: Some(count.min(100)),
        delay: Some(delay.min(10000)),
        format: query.format.clone(),
        message: query.message.clone(),
        model: query.model.clone(),
    };

    ndjson_handler(req, web::Query(params)).await
}

/// SSE endpoint with path parameters for convenience
/// GET /sse/{count} - sends {count} events with default settings
/// GET /sse/{count}/{delay} - sends {count} events with {delay}ms between events
pub async fn sse_path_handler(
    req: HttpRequest, 
    path: web::Path<(u32,)>,
    query: web::Query<SseParams>
) -> Result<HttpResponse> {
    let (count,) = path.into_inner();
    
    let params = SseParams {
        count: Some(count.min(100)),
        delay: query.delay,
        format: query.format.clone(),
        message: query.message.clone(),
        event_type: query.event_type.clone(),
    };

    sse_handler(req, web::Query(params)).await
}

/// SSE endpoint with both count and delay as path parameters
pub async fn sse_path_with_delay_handler(
    req: HttpRequest, 
    path: web::Path<(u32, u64)>,
    query: web::Query<SseParams>
) -> Result<HttpResponse> {
    let (count, delay) = path.into_inner();
    
    let params = SseParams {
        count: Some(count.min(100)),
        delay: Some(delay.min(10000)),
        format: query.format.clone(),
        message: query.message.clone(),
        event_type: query.event_type.clone(),
    };

    sse_handler(req, web::Query(params)).await
}
