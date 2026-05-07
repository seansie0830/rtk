use super::ls::{self, LsRecord};
use anyhow::Result;
use std::io::IsTerminal;
use std::path::Path;
use super::constants::NOISE_DIRS;
use colored::Colorize; // 顯式引入 Trait
use std::time::UNIX_EPOCH;

/// Fetches file information from the filesystem using native Rust std::fs.
pub fn fetch_entries(paths: &[String], show_all: bool) -> Result<(Vec<LsRecord>, Vec<LsRecord>)> {
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

        if path.is_dir() {
            for entry in std::fs::read_dir(path)?.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();

                if name == "." || name == ".." {
                    continue;
                }

                if !show_all && NOISE_DIRS.iter().any(|noise| name == *noise) {
                    continue;
                }

                if let Ok(metadata) = entry.metadata() {
                    let timestamp = Some(metadata.modified()
                                            .ok()
                                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                                            .map(|d| d.as_secs() as u64)
                                            .unwrap_or(0));
                    records.push(LsRecord {
                        extension: ls::get_extension(&name),
                        is_dir: metadata.is_dir(),
                        size: metadata.len(),
                        name,
                        timestamp
                    });
                }
            }
        } else if let Ok(metadata) = path.metadata() {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let timestamp = Some(metadata.modified()
                                    .ok()
                                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                                    .map(|d| d.as_secs() as u64)
                                    .unwrap_or(0));
            records.push(LsRecord {
                extension: ls::get_extension(&name),
                is_dir: false,
                size: metadata.len(),
                name,
                timestamp
            });
        }
    }
    let (dirs, files): (Vec<LsRecord>, Vec<LsRecord>) = records
        .into_iter() 
        .partition(|r| r.is_dir);
    Ok((dirs, files))
}

/// Entry point called by ls::run on Windows.
pub fn run_native( paths: Vec<String>, show_all: bool ,  flags:Vec<String>) -> Result<i32> {
    eprintln!("{}","⚠️ Warning: ls on Windows is not fully supported yet. some flag may not work as expected. the program use system call to fetch file information.".yellow().bold());
    let timer = crate::core::tracking::TimedExecution::start();
    
    let (mut dirs, mut files) = fetch_entries(&paths, show_all)?;
    if flags.contains(&"-r".to_string()) {
        dirs.reverse();
        files.reverse();
    }
    if flags.contains(&"-t".to_string()) {
        dirs.sort_by(|a, b| b.size.cmp(&a.size));
        files.sort_by(|a, b| b.size.cmp(&a.size));
    }
    let (entries, summary) = ls::synthesize_output(dirs,files);

    let is_tty = std::io::stdout().is_terminal();
    let output = if is_tty {
        format!("{}{}", entries, summary)
    } else {
        entries
    };

    print!("{}", output);

    timer.track(
        &format!("ls (native) {}", paths.join(" ")),
        &format!("rtk ls {}", paths.join(" ")),
        "", 
        &output,
    );

    Ok(0)
}
