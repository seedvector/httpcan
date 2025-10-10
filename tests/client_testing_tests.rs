//! HTTP Client Testing tests showing how to use HTTPCan in unit tests
//! These tests demonstrate the same patterns as the client_testing example
//! Run with: cargo test --features examples

#[cfg(feature = "examples")]
#[cfg(test)]
mod tests {
    use std::time::Duration;
    use tokio::time::timeout;

    struct TestClient {
        base_url: String,
    }

    impl TestClient {
        fn new(base_url: String) -> Self {
            Self { base_url }
        }

        async fn get_with_timeout(&self, path: &str, timeout_duration: Duration) -> Result<String, Box<dyn std::error::Error>> {
            let url = format!("{}{}", self.base_url, path);
            let client = reqwest::Client::new();
            
            let response = timeout(timeout_duration, client.get(&url).send()).await??;
            let text = response.text().await?;
            Ok(text)
        }

        async fn get_status(&self, path: &str) -> Result<u16, Box<dyn std::error::Error>> {
            let url = format!("{}{}", self.base_url, path);
            let client = reqwest::Client::new();
            
            let response = client.get(&url).send().await?;
            Ok(response.status().as_u16())
        }
    }

    // Note: These tests require a running HTTPCan server on port 8080
    // In a real application, you would typically start the server programmatically
    // or use a test framework that manages the server lifecycle

    #[tokio::test]
    #[ignore] // Ignored by default since it requires a running server
    async fn test_timeout_behavior() {
        let client = TestClient::new("http://127.0.0.1:8080".to_string());
        
        // This should timeout after 1 second, but the server delays for 5 seconds
        let result = client.get_with_timeout("/delay/5", Duration::from_secs(1)).await;
        
        assert!(result.is_err(), "Expected timeout error");
        let error_msg = result.unwrap_err().to_string();
        assert!(
            error_msg.contains("timeout") || error_msg.contains("elapsed"),
            "Error should be a timeout: {}",
            error_msg
        );
    }

    #[tokio::test]
    #[ignore] // Ignored by default since it requires a running server
    async fn test_successful_request() {
        let client = TestClient::new("http://127.0.0.1:8080".to_string());
        
        // This should succeed within the timeout
        let result = client.get_with_timeout("/delay/1", Duration::from_secs(5)).await;
        
        assert!(result.is_ok(), "Request should succeed");
        let response = result.unwrap();
        assert!(response.contains("args"), "Response should contain request info");
    }

    #[tokio::test]
    #[ignore] // Ignored by default since it requires a running server
    async fn test_status_codes() {
        let client = TestClient::new("http://127.0.0.1:8080".to_string());
        
        // Test 404 status
        let status = client.get_status("/status/404").await.unwrap();
        assert_eq!(status, 404, "Should return 404 status");
        
        // Test 200 status
        let status = client.get_status("/status/200").await.unwrap();
        assert_eq!(status, 200, "Should return 200 status");
        
        // Test 500 status
        let status = client.get_status("/status/500").await.unwrap();
        assert_eq!(status, 500, "Should return 500 status");
    }

    #[tokio::test]
    #[ignore] // Ignored by default since it requires a running server
    async fn test_json_response() {
        let client = TestClient::new("http://127.0.0.1:8080".to_string());
        
        let response = client.get_with_timeout("/json", Duration::from_secs(5)).await.unwrap();
        
        // Parse as JSON to verify it's valid
        let json: serde_json::Value = serde_json::from_str(&response).unwrap();
        
        // Check that it contains expected structure
        assert!(json.get("slideshow").is_some(), "JSON should contain slideshow");
    }

    // This test shows how you might structure tests that don't require a running server
    #[tokio::test]
    async fn test_client_configuration() {
        let client = TestClient::new("http://example.com".to_string());
        assert_eq!(client.base_url, "http://example.com");
        
        // You could test other client configuration logic here
        // without needing an actual HTTP server
    }
}

// Instructions for running these tests:
//
// These tests demonstrate the same HTTP client testing patterns as the
// client_testing example, but in a proper test framework.
//
// 1. Start HTTPCan server:
//    cargo run --example basic
//
// 2. Run the tests in another terminal:
//    cargo test --features examples -- --ignored
//
// Or run individual tests:
//    cargo test test_timeout_behavior --features examples -- --ignored
//
// Note: You can also see these patterns in action by running:
//    cargo run --example client_testing --features examples
//
