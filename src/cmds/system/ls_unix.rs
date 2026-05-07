use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    /// Matches the date+time portion in `ls -la` output, which serves as a
    /// stable anchor regardless of owner/group column width.
    /// E.g.: " Mar 31 16:18 " or " Dec 25  2024 "
    pub static ref LS_DATE_RE: Regex = Regex::new(
        r"\s+(Jan|Feb|Mar|Apr|May|Jun|Jul|Aug|Sep|Oct|Nov|Dec)\s+\d{1,2}\s+(?:\d{4}|\d{2}:\d{2})\s+"
    )
    .unwrap();
}

/// Parse a single `ls -la` line, returning `(file_type_char, size, name)`.
///
/// Uses the date field as a stable anchor — the date format in `ls -la` is
/// always three tokens (`Mon DD HH:MM` or `Mon DD  YYYY`), so we locate it
/// with a regex, then extract size (rightmost number before the date) and
/// filename (everything after the date). This handles owner/group names that
/// contain spaces, which break the old fixed-column approach.
pub fn parse_ls_line(line: &str) -> Option<(char, u64, String)> {
    let date_match = LS_DATE_RE.find(line)?;
    let name = line[date_match.end()..].to_string();

    let before_date = &line[..date_match.start()];
    let before_parts: Vec<&str> = before_date.split_whitespace().collect();
    if before_parts.len() < 4 {
        return None;
    }

    let perms = before_parts[0];
    let file_type = perms.chars().next()?;

    // Size is the rightmost parseable number before the date.
    // nlinks is also numeric but appears earlier; scanning from the end
    // guarantees we hit the size field first.
    let mut size: u64 = 0;
    for part in before_parts.iter().rev() {
        if let Ok(s) = part.parse::<u64>() {
            size = s;
            break;
        }
    }

    Some((file_type, size, name))
}
