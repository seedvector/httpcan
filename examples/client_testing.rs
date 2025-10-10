//! HTTP Client Testing example showing how to use HTTPCan for testing HTTP clients

use std::time::Duration;
use tokio::time::timeout;

// Simulate an HTTP client that we want to test
struct HttpClient {
    base_url: String,
    timeout: Duration,
}

impl HttpClient {
    fn new(base_url: String, timeout: Duration) -> Self {
        Self { base_url, timeout }
    }

    async fn get(&self, path: &str) -> Result<String, Box<dyn std::error::Error>> {
        let url = format!("{}{}", self.base_url, path);
        let client = reqwest::Client::new();
        
        let response = timeout(self.timeout, client.get(&url).send()).await??;
        let text = response.text().await?;
        Ok(text)
    }

    async fn get_with_status(&self, path: &str) -> Result<(u16, String), Box<dyn std::error::Error>> {
        let url = format!("{}{}", self.base_url, path);
        let client = reqwest::Client::new();
        
        let response = timeout(self.timeout, client.get(&url).send()).await??;
        let status = response.status().as_u16();
        let text = response.text().await?;
        Ok((status, text))
    }

    async fn get_with_headers(&self, path: &str, headers: &[(&str, &str)]) -> Result<String, Box<dyn std::error::Error>> {
        let url = format!("{}{}", self.base_url, path);
        let client = reqwest::Client::new();
        
        let mut request = client.get(&url);
        for (key, value) in headers {
            request = request.header(*key, *value);
        }
        
        let response = timeout(self.timeout, request.send()).await??;
        let text = response.text().await?;
        Ok(text)
    }
}

async fn run_integration_tests() -> Result<(), Box<dyn std::error::Error>> {
    println!("🧪 Running HTTPCan client testing examples...\n");

    // Test 1: Test slow response timeout
    println!("Test 1: Testing slow response timeout...");
    let client = HttpClient::new("http://127.0.0.1:8080".to_string(), Duration::from_secs(1));
    
    match client.get("/delay/10").await {
        Err(e) => {
            if e.to_string().contains("timeout") || e.to_string().contains("elapsed") {
                println!("✅ Test passed: Got expected timeout error");
            } else {
                println!("❌ Test failed: Got unexpected error: {}", e);
            }
        }
        Ok(_) => {
            println!("❌ Test failed: Expected timeout but request succeeded");
        }
    }

    // Test 2: Test successful request with reasonable timeout
    println!("\nTest 2: Testing successful request...");
    let client = HttpClient::new("http://127.0.0.1:8080".to_string(), Duration::from_secs(5));
    
    match client.get("/delay/1").await {
        Ok(response) => {
            println!("✅ Test passed: Got successful response");
            println!("Response preview: {}", &response[..response.len().min(100)]);
        }
        Err(e) => {
            println!("❌ Test failed: Expected success but got error: {}", e);
        }
    }

    // Test 3: Test JSON endpoint
    println!("\nTest 3: Testing JSON endpoint...");
    match client.get("/json").await {
        Ok(response) => {
            if response.contains("slideshow") {
                println!("✅ Test passed: Got expected JSON response");
            } else {
                println!("❌ Test failed: JSON response doesn't contain expected content");
            }
        }
        Err(e) => {
            println!("❌ Test failed: JSON request failed: {}", e);
        }
    }

    // Test 4: Test status codes
    println!("\nTest 4: Testing status codes...");
    match client.get_with_status("/status/404").await {
        Ok((status, _)) => {
            if status == 404 {
                println!("✅ Test passed: Got expected 404 status");
            } else {
                println!("❌ Test failed: Expected 404 but got {}", status);
            }
        }
        Err(e) => {
            println!("❌ Test failed: Status request failed: {}", e);
        }
    }

    // Test 5: Test headers echoing
    println!("\nTest 5: Testing headers endpoint...");
    let headers = &[("X-Test-Header", "test-value"), ("X-Custom", "custom-value")];
    
    match client.get_with_headers("/headers", headers).await {
        Ok(response) => {
            if response.contains("X-Test-Header") && response.contains("test-value") {
                println!("✅ Test passed: Headers are correctly echoed");
            } else {
                println!("❌ Test failed: Headers not found in response");
            }
        }
        Err(e) => {
            println!("❌ Test failed: Headers request failed: {}", e);
        }
    }

    // Test 6: Test GET endpoint
    println!("\nTest 6: Testing GET endpoint...");
    match client.get("/get?param1=value1&param2=value2").await {
        Ok(response) => {
            if response.contains("param1") && response.contains("value1") {
                println!("✅ Test passed: Query parameters are correctly handled");
            } else {
                println!("❌ Test failed: Query parameters not found in response");
            }
        }
        Err(e) => {
            println!("❌ Test failed: GET request failed: {}", e);
        }
    }

    println!("\n🎉 All integration tests completed!");
    println!("💡 This demonstrates how HTTPCan can be used as a test server for HTTP client testing");
    
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    println!("Starting HTTPCan test server on http://127.0.0.1:8080");
    println!("This example demonstrates testing HTTP clients against HTTPCan");

    // Start the server in the background using a different approach
    // In a real application, you'd typically start the server in a separate process or thread
    // For this demo, we'll show how the tests would work assuming the server is running
    
    // Uncomment the following lines to actually start the server:
    // let server = HttpCanServer::new()
    //     .port(8080)
    //     .host("127.0.0.1");
    // server.run().await?;

    // For now, let's simulate the tests with a mock explanation
    println!("📝 Note: To run these tests, start the HTTPCan server first:");
    println!("   cargo run --example basic");
    println!("   # Then in another terminal:");
    println!("   cargo run --example client_testing --features examples");
    println!();
    
    // Check if server is running and run tests
    let client = HttpClient::new("http://127.0.0.1:8080".to_string(), Duration::from_secs(2));
    
    match client.get("/get").await {
        Ok(_) => {
            println!("🚀 HTTPCan server detected! Running integration tests...\n");
            run_integration_tests().await?;
        }
        Err(_) => {
            println!("⚠️  HTTPCan server not running on port 8080");
            println!("   Start the server first with: cargo run --example basic");
            println!("   Then run this example again.");
            println!();
            println!("🔍 Here's what the tests would do:");
            println!("   1. Test timeout behavior with /delay/10 endpoint");
            println!("   2. Test successful requests with /delay/1 endpoint");
            println!("   3. Validate JSON responses from /json endpoint");
            println!("   4. Check status code handling with /status/404");
            println!("   5. Verify header echoing with /headers endpoint");
            println!("   6. Test query parameter handling with /get endpoint");
        }
    }

    Ok(())
}
