use std::io::{BufRead, BufReader};
use std::time::Duration;

use crate::mcp::types::{JsonRpcRequest, McpResponse};

/// Low-level HTTP transport for MCP JSON-RPC communication.
pub struct McpTransport {
    agent: ureq::Agent,
    endpoint: String,
}

impl McpTransport {
    pub fn new(endpoint: &str, timeout_connect: Duration, timeout_read: Duration) -> Self {
        let agent = ureq::Agent::new_with_config(
            ureq::config::Config::builder()
                .timeout_connect(Some(timeout_connect))
                .timeout_recv_body(Some(timeout_read))
                .user_agent(format!("deepwiki-dl/{}", env!("CARGO_PKG_VERSION")))
                .build(),
        );
        Self {
            agent,
            endpoint: endpoint.to_string(),
        }
    }

    /// Send a JSON-RPC request and parse the response.
    pub fn post(
        &self,
        request: &JsonRpcRequest,
        session_id: Option<&str>,
    ) -> Result<McpResponse, TransportError> {
        let body = serde_json::to_string(request).map_err(TransportError::Serialize)?;

        let mut req = self
            .agent
            .post(&self.endpoint)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");

        if let Some(sid) = session_id {
            req = req.header("Mcp-Session-Id", sid);
        }

        let response = req.send(&body).map_err(TransportError::Http)?;

        // Extract session id from response headers (case-insensitive)
        let new_session_id = response
            .headers()
            .iter()
            .find(|(name, _)| name.as_str().eq_ignore_ascii_case("mcp-session-id"))
            .and_then(|(_, value)| value.to_str().ok().map(|s| s.to_string()));

        let content_type = response
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let body = response.into_body();

        let body_value = if content_type.contains("text/event-stream") {
            let reader = BufReader::new(body.into_reader());
            parse_sse_stream(reader)?
        } else {
            let reader = body.into_reader();
            serde_json::from_reader(reader).map_err(TransportError::JsonParse)?
        };

        Ok(McpResponse {
            body: body_value,
            session_id: new_session_id,
        })
    }
}

/// Parse an SSE stream and extract JSON-RPC responses.
///
/// SSE format:
/// - Lines starting with `data:` contain data
/// - Lines starting with `:` are comments (ignored)
/// - Empty lines separate events
/// - Multiple `data:` lines in one event are joined with `\n`
pub fn parse_sse_stream<R: BufRead>(reader: R) -> Result<serde_json::Value, TransportError> {
    let mut last_valid_response: Option<serde_json::Value> = None;
    let mut current_data = String::new();

    for line in reader.lines() {
        let line = line.map_err(TransportError::Io)?;
        let line = line.trim_end_matches('\r');

        if line.starts_with(':') {
            // Comment line, ignore
            continue;
        }

        if line.is_empty() {
            // Event boundary — process accumulated data
            if !current_data.is_empty() {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(&current_data) {
                    if value.get("id").is_some() {
                        last_valid_response = Some(value);
                    }
                }
                current_data.clear();
            }
            continue;
        }

        if let Some(data) = line.strip_prefix("data:") {
            let data = data.trim_start();
            if !current_data.is_empty() {
                current_data.push('\n');
            }
            current_data.push_str(data);
        }
        // Ignore other fields (event:, id:, retry:)
    }

    // Handle final event (no trailing empty line)
    if !current_data.is_empty() {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&current_data) {
            if value.get("id").is_some() {
                last_valid_response = Some(value);
            }
        }
    }

    last_valid_response.ok_or(TransportError::NoSseResponse)
}

#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] ureq::Error),

    #[error("Failed to serialize request: {0}")]
    Serialize(serde_json::Error),

    #[error("Failed to parse JSON response: {0}")]
    JsonParse(serde_json::Error),

    #[error("No valid JSON-RPC response found in SSE stream")]
    NoSseResponse,

    #[error("I/O error reading response: {0}")]
    Io(std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sse_single_event() {
        let input = b"data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"ok\"}\n\n";
        let result = parse_sse_stream(&input[..]).unwrap();
        assert_eq!(result["id"], 1);
        assert_eq!(result["result"], "ok");
    }

    #[test]
    fn test_parse_sse_multiple_events() {
        let input = b"\
data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"first\"}\n\
\n\
data: {\"jsonrpc\":\"2.0\",\"id\":2,\"result\":\"second\"}\n\
\n";
        let result = parse_sse_stream(&input[..]).unwrap();
        assert_eq!(result["id"], 2);
        assert_eq!(result["result"], "second");
    }

    #[test]
    fn test_parse_sse_multi_data_lines() {
        let input = b"\
data: {\"jsonrpc\":\"2.0\",\n\
data: \"id\":1,\"result\":\"ok\"}\n\
\n";
        let result = parse_sse_stream(&input[..]).unwrap();
        assert_eq!(result["id"], 1);
    }

    #[test]
    fn test_parse_sse_comment_lines() {
        let input = b"\
: this is a comment\n\
data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"ok\"}\n\
: another comment\n\
\n";
        let result = parse_sse_stream(&input[..]).unwrap();
        assert_eq!(result["id"], 1);
    }

    #[test]
    fn test_parse_sse_no_trailing_newline() {
        let input = b"data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"ok\"}";
        let result = parse_sse_stream(&input[..]).unwrap();
        assert_eq!(result["id"], 1);
    }

    #[test]
    fn test_parse_sse_empty_stream() {
        let input = b"";
        assert!(parse_sse_stream(&input[..]).is_err());
    }

    #[test]
    fn test_parse_sse_only_comments() {
        let input = b": comment\n: another\n\n";
        assert!(parse_sse_stream(&input[..]).is_err());
    }

    #[test]
    fn test_parse_sse_with_event_field() {
        let input = b"\
event: message\n\
data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"ok\"}\n\
\n";
        let result = parse_sse_stream(&input[..]).unwrap();
        assert_eq!(result["id"], 1);
    }

    #[test]
    fn test_parse_sse_skip_non_jsonrpc() {
        let input = b"\
data: {\"jsonrpc\":\"2.0\",\"method\":\"progress\"}\n\
\n\
data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"ok\"}\n\
\n";
        let result = parse_sse_stream(&input[..]).unwrap();
        assert_eq!(result["id"], 1);
    }

    #[test]
    fn test_parse_sse_crlf() {
        let input = b"data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":\"ok\"}\r\n\r\n";
        let result = parse_sse_stream(&input[..]).unwrap();
        assert_eq!(result["id"], 1);
    }
}
