//! CLI commands definition

use clap::Subcommand;
use std::path::PathBuf;

/// Available CLI commands
#[derive(Subcommand)]
pub enum Commands {
    /// Start the server
    Serve {
        /// Transport mode [stdio, http, sse, hybrid]
        #[arg(short, long)]
        mode: Option<String>,

        /// Listen host
        #[arg(long)]
        host: Option<String>,

        /// Listen port
        #[arg(short, long)]
        port: Option<u16>,

        /// Enable OAuth authentication
        #[arg(long)]
        enable_oauth: Option<bool>,

        /// OAuth client ID
        #[arg(long)]
        oauth_client_id: Option<String>,

        /// OAuth client secret
        #[arg(long)]
        oauth_client_secret: Option<String>,

        /// OAuth redirect URI
        #[arg(long)]
        oauth_redirect_uri: Option<String>,

        /// Enable API Key authentication
        #[arg(long)]
        enable_api_key: Option<bool>,

        /// API Key(s) for authentication (comma-separated for multiple keys)
        #[arg(long)]
        api_keys: Option<String>,

        /// API Key header name (default: X-API-Key)
        #[arg(long)]
        api_key_header: Option<String>,

        /// Allow API Key in query parameter
        #[arg(long)]
        api_key_query_param: Option<bool>,
    },

    /// Generate API key for hashed storage
    GenerateApiKey {
        /// Key prefix used when generating the API key
        #[arg(long, default_value = "sk")]
        prefix: String,
    },

    /// List API keys from configuration file
    ListApiKeys {
        /// Configuration file path
        #[arg(short, long, default_value = "config.toml")]
        config: PathBuf,
    },

    /// Revoke an API key from configuration file
    RevokeApiKey {
        /// Configuration file path
        #[arg(short, long, default_value = "config.toml")]
        config: PathBuf,

        /// Key hash or key ID to revoke
        #[arg(short, long)]
        key: String,
    },

    /// Generate configuration file
    Config {
        /// Output file path
        #[arg(short, long, default_value = "config.toml")]
        output: PathBuf,

        /// Overwrite existing file
        #[arg(short, long)]
        force: bool,
    },

    /// Test tool
    Test {
        /// Tool to test `lookup_crate`, `search_crates`, `lookup_item`, `health_check`
        #[arg(short, long, default_value = "lookup_crate")]
        tool: String,

        /// Crate name (for `lookup_crate` and `lookup_item`)
        #[arg(long)]
        crate_name: Option<String>,

        /// Item path (for `lookup_item`)
        #[arg(long)]
        item_path: Option<String>,

        /// Search query (for `search_crates`)
        #[arg(long)]
        query: Option<String>,

        /// Search sort order (for `search_crates`): `relevance`, `downloads`, `recent-downloads`, `recent-updates`, `new`
        #[arg(long)]
        sort: Option<String>,

        /// Version number (optional)
        #[arg(long)]
        version: Option<String>,

        /// Result limit (for `search_crates`)
        #[arg(long, default_value = "10")]
        limit: u32,

        /// Output format: `json`, `markdown`, `text`
        #[arg(long, default_value = "markdown")]
        format: String,
    },

    /// Check server health status
    Health {
        /// Check type: `all`, `external`, `internal`, `docs_rs`, `crates_io`
        #[arg(short = 't', long, default_value = "all")]
        check_type: String,

        /// Verbose output
        #[arg(short, long)]
        verbose: bool,
    },

    /// Display version information
    Version,
}
