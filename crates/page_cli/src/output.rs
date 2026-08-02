use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use clap::ValueEnum;
use serde::Serialize;

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

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
    match serialize_json(value) {
        Ok(json) => {
            print!("{json}");
            0
        }
        Err(error) => {
            eprintln!("could not serialize {description}: {error}");
            1
        }
    }
}

/// Serializes a value as pretty JSON terminated by a newline.
pub fn serialize_json(value: &impl Serialize) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(value).map(|mut json| {
        json.push('\n');
        json
    })
}

/// Replaces `path` atomically after writing the complete contents to a temporary
/// file in the same directory.
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let parent = path.parent().filter(|path| !path.as_os_str().is_empty());
    let parent = parent.unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "output path has no file name")
    })?;

    for _ in 0..100 {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let mut temporary_name = OsString::from(".");
        temporary_name.push(file_name);
        temporary_name.push(format!(".page-{}-{sequence}.tmp", std::process::id()));
        let temporary_path = parent.join(temporary_name);

        let mut temporary = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        };

        if let Err(error) = temporary
            .write_all(contents)
            .and_then(|()| temporary.sync_all())
        {
            drop(temporary);
            let _ = fs::remove_file(&temporary_path);
            return Err(error);
        }
        drop(temporary);

        if let Err(error) = fs::rename(&temporary_path, path) {
            let _ = fs::remove_file(&temporary_path);
            return Err(error);
        }
        return Ok(());
    }

    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not allocate a temporary output file",
    ))
}
