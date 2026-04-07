pub mod transport;
pub mod types;

use std::time::Duration;

use crate::mcp::transport::{McpTransport, TransportError};
use crate::mcp::types::{JsonRpcRequest, JsonRpcResponse, ToolResult};

const DEFAULT_ENDPOINT: &str = "https://mcp.deepwiki.com/mcp";
const PROTOCOL_VERSION: &str = "2025-03-26";
const MAX_RETRIES: u32 = 3;
const RETRY_DELAYS: [u64; 3] = [1, 2, 4];

/// MCP client for DeepWiki API.
pub struct McpClient {
    transport: McpTransport,
    session_id: Option<String>,
    next_id: u64,
}

impl McpClient {
    /// Connect to MCP server: performs initialize handshake.
    pub fn connect(
        endpoint: Option<&str>,
        timeout_connect: Duration,
        timeout_read: Duration,
    ) -> Result<Self, McpError> {
        let endpoint = endpoint.unwrap_or(DEFAULT_ENDPOINT);
        let transport = McpTransport::new(endpoint, timeout_connect, timeout_read);

        let mut client = Self {
            transport,
            session_id: None,
            next_id: 1,
        };

        client.handshake()?;
        Ok(client)
    }

    fn handshake(&mut self) -> Result<(), McpError> {
        // Step 1: initialize
        let init_request = JsonRpcRequest::new(
            self.next_id(),
            "initialize",
            serde_json::json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {
                    "name": "deepwiki-dl",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }),
        );

        let response = self
            .transport
            .post(&init_request, None)
            .map_err(|e| McpError::HandshakeFailed {
                message: format!("initialize request failed: {e}"),
            })?;

        // Check for JSON-RPC error in response
        let rpc_resp: JsonRpcResponse = serde_json::from_value(response.body)
            .map_err(|e| McpError::HandshakeFailed {
                message: format!("failed to parse initialize response: {e}"),
            })?;

        if let Some(err) = rpc_resp.error {
            return Err(McpError::RpcError {
                code: err.code,
                message: err.message,
            });
        }

        // Save session id
        self.session_id = response.session_id;

        // Step 2: notifications/initialized (fire-and-forget)
        let notif = JsonRpcRequest::notification(
            "notifications/initialized",
            serde_json::json!({}),
        );
        // Best effort — ignore errors
        let _ = self.transport.post(&notif, self.session_id.as_deref());

        Ok(())
    }

    /// Call an MCP tool with retry logic.
    pub fn call_tool(
        &mut self,
        tool_name: &str,
        arguments: serde_json::Value,
    ) -> Result<String, McpError> {
        let mut last_error = None;

        for attempt in 0..MAX_RETRIES {
            if attempt > 0 {
                let delay = RETRY_DELAYS[attempt as usize - 1];
                std::thread::sleep(Duration::from_secs(delay));
            }

            let request = JsonRpcRequest::new(
                self.next_id(),
                "tools/call",
                serde_json::json!({
                    "name": tool_name,
                    "arguments": arguments,
                }),
            );

            match self.transport.post(&request, self.session_id.as_deref()) {
                Ok(response) => {
                    let rpc_resp: JsonRpcResponse = serde_json::from_value(response.body)
                        .map_err(|e| McpError::Transport {
                            message: format!("failed to parse response: {e}"),
                        })?;

                    if let Some(err) = rpc_resp.error {
                        return Err(McpError::RpcError {
                            code: err.code,
                            message: err.message,
                        });
                    }

                    let result_value = rpc_resp.result.ok_or_else(|| McpError::Transport {
                        message: "response has neither result nor error".to_string(),
                    })?;

                    let tool_result: ToolResult = serde_json::from_value(result_value)
                        .map_err(|e| McpError::Transport {
                            message: format!("failed to parse tool result: {e}"),
                        })?;

                    if tool_result.is_error {
                        return Err(McpError::ToolError {
                            tool: tool_name.to_string(),
                            message: tool_result.text(),
                        });
                    }

                    return Ok(tool_result.text());
                }
                Err(e) => {
                    if !is_retryable(&e) {
                        return Err(McpError::Transport {
                            message: e.to_string(),
                        });
                    }
                    last_error = Some(e);
                }
            }
        }

        Err(McpError::Transport {
            message: format!(
                "failed after {MAX_RETRIES} retries: {}",
                last_error
                    .map(|e| e.to_string())
                    .unwrap_or_else(|| "unknown error".to_string())
            ),
        })
    }

    /// Read wiki structure for a repository.
    pub fn read_wiki_structure(&mut self, repo: &str) -> Result<String, McpError> {
        self.call_tool("read_wiki_structure", serde_json::json!({ "repoName": repo }))
    }

    /// Read wiki contents for a repository.
    pub fn read_wiki_contents(&mut self, repo: &str) -> Result<String, McpError> {
        self.call_tool("read_wiki_contents", serde_json::json!({ "repoName": repo }))
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

fn is_retryable(err: &TransportError) -> bool {
    matches!(
        err,
        TransportError::Http(_) | TransportError::Io(_) | TransportError::NoSseResponse
    )
}

#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("MCP handshake failed: {message}")]
    HandshakeFailed { message: String },

    #[error("Repository not indexed by DeepWiki: {repo}")]
    RepoNotFound { repo: String },

    #[error("JSON-RPC error {code}: {message}")]
    RpcError { code: i64, message: String },

    #[error("Tool call failed ({tool}): {message}")]
    ToolError { tool: String, message: String },

    #[error("Response too large ({size} bytes, max {max} bytes). Try --pages to fetch specific sections.")]
    ResponseTooLarge { size: u64, max: u64 },

    #[error("Transport error: {message}")]
    Transport { message: String },
}
