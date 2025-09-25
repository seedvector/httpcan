# HTTPCan

A simple HTTP request & response service built with Rust and Actix Web, with httpbin compatibility.

## Features

- **HTTPBin Compatible** - Full compatibility with httpbin API for seamless migration and testing
- **AI-Era Streaming** - Native support for SSE and NDJSON endpoints with OpenAI/Ollama format compatibility
- **Tiny Docker Image** - Less than 10MB image size for fast deployment
- **Minimal Memory Footprint** - Extremely low memory usage for efficient resource utilization
- **High Throughput** - Built with Rust and async I/O for maximum performance

This server implements the HTTPBin API with the following endpoints:

### HTTP Methods
- `GET /get` - Returns request data for GET requests
- `POST /post` - Returns request data for POST requests  
- `PUT /put` - Returns request data for PUT requests
- `PATCH /patch` - Returns request data for PATCH requests
- `DELETE /delete` - Returns request data for DELETE requests

### Anything Endpoints (Multiple HTTP Methods)
- `/anything` - Accepts GET, POST, PUT, PATCH, DELETE, TRACE
- `/anything/{anything}` - Same as above with path parameter

### Authentication
- `GET /basic-auth/{user}/{passwd}` - Basic authentication
- `GET /basic-auth/{user}` - Basic authentication with username only (empty password)
- `GET /hidden-basic-auth/{user}/{passwd}` - Basic auth with 404 on failure
- `GET /hidden-basic-auth/{user}` - Basic auth with username only, 404 on failure
- `GET /bearer` - Bearer token authentication
- `GET /digest-auth/{qop}/{user}/{passwd}` - Digest authentication
- `GET /digest-auth/{qop}/{user}/{passwd}/{algorithm}` - Digest auth with algorithm
- `GET /digest-auth/{qop}/{user}/{passwd}/{algorithm}/{stale_after}` - Full digest auth

### Response Formats
- `GET /json` - Returns JSON response
- `GET /xml` - Returns XML response
- `GET /html` - Returns HTML response
- `GET /robots.txt` - Returns robots.txt
- `GET /deny` - Returns denied message
- `GET /encoding/utf8` - Returns UTF-8 encoded response
- `GET /gzip` - Returns gzip-compressed response
- `GET /deflate` - Returns deflate-compressed response
- `GET /brotli` - Returns brotli-compressed response

### Dynamic Data
- `GET /uuid` - Returns a UUID4
- `GET /base64/{value}` - Decodes base64-encoded string
- `GET /bytes/{n}` - Returns n random bytes
- `GET /stream-bytes/{n}` - Streams n random bytes
- `GET /stream/{n}` - Streams n JSON responses
- `GET /range/{numbytes}` - Returns bytes with range support
- `GET /links/{n}/{offset}` - Returns page with n links
- `GET /drip` - Drips data over time
- `/delay/{delay}` - Returns delayed response (supports multiple methods)

### Redirects
- `GET /redirect/{n}` - 302 redirects n times
- `GET /relative-redirect/{n}` - Relative 302 redirects n times  
- `GET /absolute-redirect/{n}` - Absolute 302 redirects n times
- `/redirect-to` - 302 redirects to given URL (supports multiple methods)

### Request Inspection
- `GET /headers` - Returns request headers
- `GET /ip` - Returns client IP address
- `GET /user-agent` - Returns User-Agent header

### Response Inspection
- `GET /cache` - Returns 304 if caching headers present
- `GET /cache/{value}` - Sets Cache-Control header
- `GET /etag/{etag}` - Returns given ETag
- `GET /response-headers` - Returns custom response headers from query
- `POST /response-headers` - Returns custom response headers from query

### Cookies
- `GET /cookies` - Returns cookies
- `GET /cookies/set` - Sets cookies from query string
- `GET /cookies/set/{name}/{value}` - Sets specific cookie
- `GET /cookies/delete` - Deletes cookies from query string

### Images
- `GET /image` - Returns image based on Accept header
- `GET /image/png` - Returns PNG image
- `GET /image/jpeg` - Returns JPEG image
- `GET /image/webp` - Returns WebP image
- `GET /image/svg` - Returns SVG image

### Status Codes
- `/status/{codes}` - Returns given status code or random from list (supports multiple methods)

### Streaming Endpoints
- `GET /sse` - Server-Sent Events endpoint with configurable event count, delay, and format
- `GET /sse/{count}` - SSE with specified event count
- `GET /sse/{count}/{delay}` - SSE with specified event count and delay
- `GET /ndjson` - NDJSON streaming endpoint with configurable parameters
- `GET /ndjson/{count}` - NDJSON with specified event count  
- `GET /ndjson/{count}/{delay}` - NDJSON with specified event count and delay

## Usage

### Command Line Arguments

```bash
httpcan [OPTIONS]
```

**Options:**
- `-p, --port <PORT>` - Port number to listen on (default: 8080)
- `--no-current-server` - Do not add current server to OpenAPI specification servers list
- `--exclude-headers <HEADERS>` - Exclude specific headers from responses. Comma-separated list of header keys, supports wildcard suffix matching (e.g., "foo, x-bar-*"). Built-in filtering for Nginx, Cloudflare, AWS, GCP, and Azure headers
- `-h, --help` - Print help information
- `-V, --version` - Print version information

### Start the server
```bash
# Default port 8080
cargo run

# Custom port
cargo run -- --port 3000

# Exclude headers
cargo run -- --exclude-headers "foo, x-bar-*"

# Multiple options
cargo run -- --port 3000 --no-current-server --exclude-headers "foo, x-bar-*"
```

The server will start on `http://0.0.0.0:8080` (or specified port)

### Example requests
```bash
# Basic GET request
curl http://localhost:8080/get

# POST with JSON data
curl -X POST http://localhost:8080/post \
  -H "Content-Type: application/json" \
  -d '{"key": "value"}'

# Multiple HTTP methods on same endpoint  
curl -X PUT http://localhost:8080/anything
curl -X DELETE http://localhost:8080/anything

# Generate UUID
curl http://localhost:8080/uuid

# Basic authentication
curl -u username:password http://localhost:8080/basic-auth/username/password

# Basic authentication with username only (empty password)
curl -u username: http://localhost:8080/basic-auth/username

# Get compressed response
curl -H "Accept-Encoding: gzip" http://localhost:8080/gzip

# Status codes
curl http://localhost:8080/status/418
curl http://localhost:8080/status/200,404,500  # Random selection

# Server-Sent Events
curl http://localhost:8080/sse?count=3&format=simple
curl http://localhost:8080/sse/5?format=openai&delay=2000
curl http://localhost:8080/sse?format=custom&message="Hello%20World"

# NDJSON streaming
curl http://localhost:8080/ndjson?count=3&format=simple
curl http://localhost:8080/ndjson/5?format=ollama&model=llama3&delay=1500
curl http://localhost:8080/ndjson?format=openai&count=2
```

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
