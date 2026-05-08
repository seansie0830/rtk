use crate::cmds::system::ls_format::LsRecordType;
use super::constants::NOISE_DIRS;
use super::ls::{self, LsRecord};

use anyhow::Result;
use colored::Colorize; 
use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::Path;
use std::time::UNIX_EPOCH;


/// Fetches file information from the filesystem using native Rust std::fs.
/// refactor required
pub fn fetch_entries(paths: &[String], show_all: bool) -> Result<Vec<LsRecord>> {
    let mut records = Vec::new();
    let targets: Vec<String> = if paths.is_empty() {
        vec![".".to_string()]
    } else {
        paths.to_vec()
    };

    for path_str in &targets {
        let path = Path::new(path_str);
        if !path.exists() {
            eprintln!("rtk: {}: No such file or directory", path_str);
            continue;
        }

        let metadata = match std::fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if metadata.is_dir() {
            for entry_res in std::fs::read_dir(path)? {
                let entry = match entry_res {
                    Ok(e) => e,
                    Err(_) => continue,
                };
                let name = entry.file_name().to_string_lossy().to_string();

                if name == "." || name == ".." {
                    continue;
                }

                if !show_all && NOISE_DIRS.iter().any(|noise| name == *noise) {
                    continue;
                }

                if let Ok(file_type) = entry.file_type() {
                    let ls_file_type = if file_type.is_symlink() {
                        LsRecordType::SYMBOLINK
                    } else if file_type.is_dir() {
                        LsRecordType::DIRECTORY
                    } else {
                        LsRecordType::FILE
                    };

                    let meta = entry.metadata().or_else(|_| std::fs::symlink_metadata(entry.path())).ok();
                    let (size, timestamp) = if let Some(m) = meta {
                        let ts = m.modified()
                            .ok()
                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                            .map(|d| d.as_secs() as u64)
                            .unwrap_or(0);
                        (m.len(), Some(ts))
                    } else {
                        (0, None)
                    };

                    records.push(LsRecord {
                        extension: ls::get_extension(&name),
                        file_type: ls_file_type,
                        size,
                        name,
                        timestamp,
                    });
                }
            }
        } else {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
                
            let ls_file_type = if metadata.is_symlink() {
                LsRecordType::SYMBOLINK
            } else {
                LsRecordType::FILE
            };

            let timestamp = Some(
                metadata
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs() as u64)
                    .unwrap_or(0),
            );
            records.push(LsRecord {
                extension: ls::get_extension(&name),
                file_type: ls_file_type,
                size: metadata.len(),
                name,
                timestamp,
            });
        }
    }
    Ok(records)
}
fn warn_unsupported_flags(flags: &[String]) {
    let allowed_flags = ["-t", "-r", "-rt", "-tr"];

    let unsupported_flags: Vec<&String> = flags
        .iter()
        .filter(|f| !allowed_flags.contains(&f.as_str()))
        .collect();

    if !unsupported_flags.is_empty() {
        eprintln!(
            "{}",
            format!(
                "rtk ls: native Windows path ignores flags: {:?}",
                unsupported_flags
            )
            .bold()
            .yellow()
        );
    }
}

pub fn run_native(paths: Vec<String>, show_all: bool, flags: Vec<String>) -> Result<(i32, String)> {
    warn_unsupported_flags(&flags);

    let active_flags: HashSet<char> = flags
        .iter()
        .filter(|f| f.starts_with('-') && !f.starts_with("--"))
        .flat_map(|f| f.chars().skip(1)) // 跳過 '-'
        .collect();

    let is_r = active_flags.contains(&'r');
    let is_t = active_flags.contains(&'t');

    let mut records = fetch_entries(&paths, show_all)?;

    let sort_fn = if is_t {
        //
        |a: &LsRecord, b: &LsRecord| b.timestamp.unwrap_or(0).cmp(&a.timestamp.unwrap_or(0))
    } else {
        //
        |a: &LsRecord, b: &LsRecord| a.name.cmp(&b.name)
    };

    records.sort_by(sort_fn);

    // 
    if is_r {
        records.reverse();
    }

    let (entries, summary) = ls::synthesize_output(records);
    let is_tty = std::io::stdout().is_terminal();
    let output = if is_tty {
        format!("{}{}", entries, summary)
    } else {
        entries
    };

    Ok((0, output))
}
