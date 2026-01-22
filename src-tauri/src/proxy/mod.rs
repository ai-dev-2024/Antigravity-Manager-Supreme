// proxy Module - API Anti-generationalService

// existingModule (reserve)
pub mod config;
pub mod token_manager;
pub mod project_resolver;
pub mod server;
pub mod security;

// new architectureModule
pub mod mappers;           // ProtocolConverter
pub mod handlers;          // API Endpoint handler
pub mod middleware;        // Axum Middleware
pub mod upstream;          // upstreamClient
pub mod common;            // publicTool
pub mod providers;         // Extra upstream providers (z.ai, etc.)
pub mod zai_vision_mcp;    // Built-in Vision MCP server state
pub mod zai_vision_tools;  // Built-in Vision MCP tools (z.ai vision API)
pub mod monitor;           // monitor
pub mod rate_limit;        // Rate LimitTrace
pub mod sticky_config;     // Sticky schedulingConfig
pub mod session_manager;   // SessionFingerprint management
pub mod audio;             // AudioHandleModule
pub mod signature_cache;   // Signature Cache (v3.3.16)
pub mod cli_sync;          // CLI ConfigSync (v3.3.35)


pub use config::ProxyConfig;
pub use config::ProxyAuthMode;
pub use config::ZaiConfig;
pub use config::ZaiDispatchMode;
pub use token_manager::TokenManager;
pub use server::AxumServer;
pub use security::ProxySecurityConfig;
pub use signature_cache::SignatureCache;

#[cfg(test)]
pub mod tests;
