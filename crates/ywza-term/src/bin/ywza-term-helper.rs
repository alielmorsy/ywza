//! Standalone helper binary (used by the integration tests); the agent normally re-invokes itself.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    std::process::exit(ywza_term::helper_main(&args).unwrap_or(2));
}
