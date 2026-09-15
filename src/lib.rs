//! Walks a directory tree and reports how much space each entry uses.
//!
//! The scan always walks the full tree (sizes have to be exact), but display
//! can be limited separately via depth/top so a huge tree doesn't flood the
//! terminal.

use std::fs;
use std::path::Path;

/// One file or directory in the scanned tree.
///
/// `children` is always sorted largest-first so callers can truncate the
/// list for display without re-sorting.
pub struct Entry {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
    pub children: Vec<Entry>,
    /// Set when a path couldn't be read (permission denied, race with a
    /// deletion, etc). The entry is kept in the tree with size 0 rather than
    /// dropped, so totals stay honest about what's missing.
    pub error: Option<String>,
}

/// Scans `path` and returns the root entry for the tree rooted there.
///
/// `excludes` is a list of glob patterns matched against each entry's bare
/// name (not its full path) as it's encountered; a match skips the entry
/// entirely, so it's excluded from both the printed tree and the size
/// totals of its ancestors. The root itself is never checked against
/// `excludes` - if you don't want it scanned, don't pass it.
pub fn scan(path: &Path, excludes: &[String]) -> Entry {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    scan_inner(path, name, excludes)
}

fn scan_inner(path: &Path, name: String, excludes: &[String]) -> Entry {
    // symlink_metadata (not metadata) so we see symlinks themselves instead
    // of silently following them into who-knows-where, or looping forever
    // on a self-referential link.
    let meta = match fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(e) => {
            return Entry {
                name,
                size: 0,
                is_dir: false,
                children: Vec::new(),
                error: Some(e.to_string()),
            }
        }
    };

    if meta.is_dir() {
        let mut children = Vec::new();
        let mut total: u64 = 0;
        let mut error = None;

        match fs::read_dir(path) {
            Ok(read_dir) => {
                for item in read_dir {
                    match item {
                        Ok(dir_entry) => {
                            let child_name = dir_entry.file_name().to_string_lossy().into_owned();
                            if excludes.iter().any(|p| glob_match(p, &child_name)) {
                                continue;
                            }
                            let child = scan_inner(&dir_entry.path(), child_name, excludes);
                            total += child.size;
                            children.push(child);
                        }
                        Err(e) => error = Some(e.to_string()),
                    }
                }
            }
            Err(e) => error = Some(e.to_string()),
        }

        children.sort_by(|a, b| b.size.cmp(&a.size));

        return Entry {
            name,
            size: total,
            is_dir: true,
            children,
            error,
        };
    }

    // Files and symlinks both report a length via symlink_metadata; other
    // kinds (sockets, devices) fall through with size 0.
    let size = if meta.is_file() || meta.is_symlink() {
        meta.len()
    } else {
        0
    };

    Entry {
        name,
        size,
        is_dir: false,
        children: Vec::new(),
        error: None,
    }
}

/// Matches `name` against a shell-style glob pattern: `*` stands for any run
/// of characters (including none), `?` for exactly one, everything else is
/// literal. No brace or bracket expansion - that keeps matching predictable
/// for the odd characters that show up in real filenames.
fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    glob_match_chars(&p, &n)
}

fn glob_match_chars(p: &[char], n: &[char]) -> bool {
    match p.first() {
        None => n.is_empty(),
        Some('*') => {
            glob_match_chars(&p[1..], n) || (!n.is_empty() && glob_match_chars(p, &n[1..]))
        }
        Some('?') => !n.is_empty() && glob_match_chars(&p[1..], &n[1..]),
        Some(c) => !n.is_empty() && n[0] == *c && glob_match_chars(&p[1..], &n[1..]),
    }
}

/// Formats a byte count the way a human expects to read it (base 1024,
/// one decimal place above the smallest unit).
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];

    if bytes < 1024 {
        return format!("{} {}", bytes, UNITS[0]);
    }

    let mut size = bytes as f64;
    let mut unit_index = 0;
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    format!("{:.1} {}", size, UNITS[unit_index])
}

/// Prints `entry` as an indented tree.
///
/// `max_depth` stops descending past that depth (the totals shown are still
/// exact, since scanning already summed the full subtree). `top` caps how
/// many children of each directory are shown, folding the rest into a
/// single "N more" line.
pub fn print_tree(entry: &Entry, depth: usize, max_depth: Option<usize>, top: Option<usize>) {
    let indent = "  ".repeat(depth);
    println!("{}{:>10}  {}", indent, format_size(entry.size), entry.name);

    if let Some(err) = &entry.error {
        println!("{}  ! {}", indent, err);
    }

    if !entry.is_dir {
        return;
    }
    if let Some(max) = max_depth {
        if depth >= max {
            return;
        }
    }

    let shown = match top {
        Some(n) => n.min(entry.children.len()),
        None => entry.children.len(),
    };

    for child in entry.children.iter().take(shown) {
        print_tree(child, depth + 1, max_depth, top);
    }

    let remaining = entry.children.len() - shown;
    if remaining > 0 {
        let remaining_size: u64 = entry.children[shown..].iter().map(|c| c.size).sum();
        let child_indent = "  ".repeat(depth + 1);
        println!(
            "{}{:>10}  ({} more)",
            child_indent,
            format_size(remaining_size),
            remaining
        );
    }
}

impl Entry {
    /// Serializes the full tree (no depth/top limiting - JSON output is for
    /// scripts, which can filter it themselves).
    pub fn to_json(&self) -> String {
        let mut out = String::new();
        self.write_json(&mut out);
        out
    }

    fn write_json(&self, out: &mut String) {
        out.push('{');
        out.push_str("\"name\":\"");
        escape_json_into(&self.name, out);
        out.push_str("\",\"size\":");
        out.push_str(&self.size.to_string());
        out.push_str(",\"is_dir\":");
        out.push_str(if self.is_dir { "true" } else { "false" });

        if let Some(err) = &self.error {
            out.push_str(",\"error\":\"");
            escape_json_into(err, out);
            out.push('"');
        }

        if self.is_dir {
            out.push_str(",\"children\":[");
            for (i, child) in self.children.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                child.write_json(out);
            }
            out.push(']');
        }

        out.push('}');
    }
}

fn escape_json_into(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_size_below_1024_has_no_decimal() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
    }

    #[test]
    fn format_size_scales_units() {
        assert_eq!(format_size(1024), "1.0 KiB");
        assert_eq!(format_size(1536), "1.5 KiB");
        assert_eq!(format_size(1024 * 1024), "1.0 MiB");
    }

    #[test]
    fn escape_json_handles_quotes_and_control_chars() {
        let mut out = String::new();
        escape_json_into("a \"quoted\"\tname", &mut out);
        assert_eq!(out, "a \\\"quoted\\\"\\tname");
    }

    #[test]
    fn glob_match_literal() {
        assert!(glob_match("target", "target"));
        assert!(!glob_match("target", "targets"));
        assert!(!glob_match("target", "tar"));
    }

    #[test]
    fn glob_match_star() {
        assert!(glob_match("*.log", "debug.log"));
        assert!(glob_match("*.log", ".log"));
        assert!(!glob_match("*.log", "debug.log.gz"));
        assert!(glob_match("node_*", "node_modules"));
        assert!(glob_match("*", ""));
        assert!(glob_match("*", "anything"));
    }

    #[test]
    fn glob_match_question_mark() {
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("a?c", "ac"));
        assert!(!glob_match("a?c", "abbc"));
    }

    #[test]
    fn glob_match_mixed_wildcards() {
        assert!(glob_match("*.?", "file.a"));
        assert!(!glob_match("*.?", "file.ab"));
    }
}
