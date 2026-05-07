use std::collections::HashMap;

/// Represents a single directory entry for token-optimized listing.
pub struct LsRecord {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub extension: String,
    pub timestamp: Option<u64>,
}

/// Format bytes into human-readable size
pub fn human_size(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1}M", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1}K", bytes as f64 / 1024.0)
    } else {
        format!("{}B", bytes)
    }
}

/// Synthesizes the compact, token-optimized string from a list of records.
pub fn synthesize_output(dir_and_files: (Vec<LsRecord>, Vec<LsRecord>)) -> (String, String) {
    let (mut dirs, mut files) = dir_and_files;
    if dirs.is_empty() && files.is_empty() {
        return ("(empty)\n".to_string(), String::new());
    }

    let mut by_ext = HashMap::new();

    for file in &files {
        *by_ext.entry(file.extension.clone()).or_insert(0) += 1;
    }

    // Sort to ensure stable output order
    #[cfg(not(target_os = "windows"))]{
        dirs.sort_by(|a, b| a.name.cmp(&b.name));
        files.sort_by(|a, b| a.name.cmp(&b.name));  
    }

    let mut entries = String::new();
    for d in &dirs {
        entries.push_str(&format!("{}/\n", d.name));
    }
    for f in &files {
        entries.push_str(&format!("{}  {}\n", f.name, human_size(f.size)));
    }

    // Summary line (separate so caller can suppress when piped)
    let mut summary = format!("\nSummary: {} files, {} dirs", files.len(), dirs.len());
    if !by_ext.is_empty() {
        let mut ext_counts: Vec<_> = by_ext.iter().collect();
        ext_counts.sort_by(|a, b| b.1.cmp(a.1).then_with(|| a.0.cmp(b.0)));
        let ext_parts: Vec<String> = ext_counts
            .iter()
            .take(5)
            .map(|(ext, count)| format!("{} {}", count, ext))
            .collect();
        summary.push_str(" (");
        summary.push_str(&ext_parts.join(", "));
        if ext_counts.len() > 5 {
            summary.push_str(&format!(", +{} more", ext_counts.len() - 5));
        }
        summary.push(')');
    }
    summary.push('\n');

    (entries, summary)
}
