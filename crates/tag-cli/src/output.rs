use clap::ValueEnum;
use serde::Serialize;

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
pub enum ReportFormat {
    #[default]
    Text,
    Json,
}

/// Serializes `value` as pretty JSON to stdout, or prints
/// `could not serialize {description}: ...` to stderr on failure.
/// Returns `0` on success and `1` on a serialization failure.
pub fn emit_json(value: &impl Serialize, description: &str) -> i32 {
    match serde_json::to_string_pretty(value) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(error) => {
            eprintln!("could not serialize {description}: {error}");
            1
        }
    }
}
