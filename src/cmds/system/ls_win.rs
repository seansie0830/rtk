use super::constants::NOISE_DIRS;
use super::ls::{self, LsRecord, LsRecordType};

use anyhow::Result;
use colored::Colorize; 
use std::collections::HashSet;
use std::io::IsTerminal;
use std::path::Path;
use std::time::UNIX_EPOCH;

pub fn estimate_raw_dir_output(records: &[LsRecord]) -> String {
    let mut chars = 8; // "total 0\n"
    for r in records {
        // Heuristic: ~50 chars of fixed `ls -la` metadata overhead + filename length
        chars += 45 + r.name.len();
    }
    " ".repeat(chars)
}

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

pub fn run_native(paths: Vec<String>, show_all: bool, flags: Vec<String>) -> Result<(i32, String, String)> {
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

    let raw_estimate = estimate_raw_dir_output(&records);

    let (entries, summary) = super::ls_format::synthesize_output(records);
    let is_tty = std::io::stdout().is_terminal();
    let output = if is_tty {
        format!("{}{}", entries, summary)
    } else {
        entries
    };

    Ok((0, output, raw_estimate))
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::{self, File};
    use crate::cmds::system::ls_format::synthesize_output;

    #[test]
    fn test_fetch_entries_basic() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        fs::create_dir(dir_path.join("src")).unwrap();
        File::create(dir_path.join("Cargo.toml")).unwrap();
        File::create(dir_path.join("README.md")).unwrap();

        let records = fetch_entries(&[dir_path.to_string_lossy().into_owned()], false).unwrap();
        let (entries, _summary) = synthesize_output(records);

        assert!(entries.contains("src/"));
        assert!(entries.contains("Cargo.toml"));
        assert!(entries.contains("README.md"));
        assert!(!entries.contains("total"));
    }

    #[test]
    fn test_fetch_entries_filters_noise() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        fs::create_dir(dir_path.join("node_modules")).unwrap();
        fs::create_dir(dir_path.join(".git")).unwrap();
        fs::create_dir(dir_path.join("target")).unwrap();
        fs::create_dir(dir_path.join("src")).unwrap();
        File::create(dir_path.join("main.rs")).unwrap();

        let records = fetch_entries(&[dir_path.to_string_lossy().into_owned()], false).unwrap();
        let (entries, _summary) = synthesize_output(records);

        assert!(!entries.contains("node_modules"));
        assert!(!entries.contains(".git"));
        assert!(!entries.contains("target"));
        assert!(entries.contains("src/"));
        assert!(entries.contains("main.rs"));
    }

    #[test]
    fn test_fetch_entries_show_all() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        fs::create_dir(dir_path.join(".git")).unwrap();
        fs::create_dir(dir_path.join("src")).unwrap();

        let records = fetch_entries(&[dir_path.to_string_lossy().into_owned()], true).unwrap();
        let (entries, _summary) = synthesize_output(records);

        assert!(entries.contains(".git/"));
        assert!(entries.contains("src/"));
    }

    #[test]
    fn test_fetch_entries_empty() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        let records = fetch_entries(&[dir_path.to_string_lossy().into_owned()], false).unwrap();
        let (entries, summary) = synthesize_output(records);

        assert_eq!(entries, "(empty)\n");
        assert!(summary.is_empty());
    }

    #[test]
    fn test_fetch_entries_symlinks() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        let target_path = dir_path.join("target.txt");
        File::create(&target_path).unwrap();

        let link_path = dir_path.join("link.txt");
        
        // Handling symlinks specifically on windows 
        #[cfg(windows)]
        let symlink_result = std::os::windows::fs::symlink_file(&target_path, &link_path);
        
        #[cfg(not(windows))]
        let symlink_result = std::os::unix::fs::symlink(&target_path, &link_path);

        if let Err(e) = symlink_result {
            // Ignore error if symlink creation failed due to lack of administrative privileges on Windows
            eprintln!("Failed to create symlink: {:?}", e);
            return;
        }

        let records = fetch_entries(&[dir_path.to_string_lossy().into_owned()], false).unwrap();
        let (entries, _summary) = synthesize_output(records);

        // NOTE: This assertion will FAIL with your current code, as fetch_entries 
        // does not append " -> target.txt" to the file name. 
        assert!(
            entries.contains("link.txt -> target.txt") || entries.contains(&format!("link.txt -> {}", target_path.to_string_lossy())),
            "Symlink output does not include target, got entries:\n{}", 
            entries
        );
    }

    #[test]
    fn test_fetch_entries_summary() {
        let dir = tempdir().unwrap();
        let dir_path = dir.path();

        fs::create_dir(dir_path.join("src")).unwrap();
        File::create(dir_path.join("main.rs")).unwrap();
        File::create(dir_path.join("lib.rs")).unwrap();
        File::create(dir_path.join("Cargo.toml")).unwrap();

        let records = fetch_entries(&[dir_path.to_string_lossy().into_owned()], false).unwrap();
        let (_entries, summary) = synthesize_output(records);

        assert!(summary.contains("Summary: 3 files, 1 dirs"));
        assert!(summary.contains(".rs"));
        assert!(summary.contains(".toml"));
    }
}

