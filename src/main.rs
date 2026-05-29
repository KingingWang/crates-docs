//! Crates Docs MCP Server main program

use clap::Parser;
use crates_docs::cli::{run, Cli};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Restore the default SIGPIPE disposition so that piping CLI output into
    // tools like `head` or `less` terminates the process cleanly instead of
    // panicking with "failed printing to stdout: Broken pipe" (exit code 101).
    reset_sigpipe();
    let cli = Cli::parse();
    run(cli).await
}

/// Reset SIGPIPE to its default action on Unix so broken pipes do not panic.
#[cfg(unix)]
fn reset_sigpipe() {
    // SAFETY: setting a signal handler to the default disposition is a simple,
    // well-defined libc call with no memory-safety implications.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }
}

/// No-op on non-Unix platforms, which do not have SIGPIPE.
#[cfg(not(unix))]
fn reset_sigpipe() {}
