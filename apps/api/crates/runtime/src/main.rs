//! Launcher. Everything it needs lives in the library beside it.
//!
//! Kept this thin on purpose: a binary that only starts a runtime and calls
//! one function has nothing in it worth testing, and everything that *is*
//! worth testing sits in a library that `tests/` and other crates can reach.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    anakmobil_runtime::run().await
}
