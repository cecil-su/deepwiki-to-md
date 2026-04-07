pub mod filter;

use std::sync::OnceLock;

use regex::Regex;

use crate::types::WikiPageMeta;

fn numbered_entry_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(\d+(?:\.\d+)*)\s+(.+)$").unwrap())
}

fn page_separator_regex() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^\s*#\s+Page:\s*(.+?)\s*$").unwrap())
}

/// Convert a title to a URL-friendly slug.
pub fn slugify(title: &str) -> String {
    title
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-")
}

/// Parse the text returned by `read_wiki_structure` into structured page metadata.
///
/// Expected format (from MCP API):
/// ```text
/// Available pages for owner/repo:
///
/// - 1 Overview
///   - 1.1 Repository Structure
///   - 1.2 Getting Started
/// - 2 Core Systems
///   - 2.1 Architecture
/// ```
pub fn parse_wiki_structure(text: &str) -> Vec<WikiPageMeta> {
    let mut pages = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();

        // Skip empty lines and header lines
        if trimmed.is_empty() || trimmed.starts_with("Available pages") {
            continue;
        }

        // Strip leading "- " if present
        let content = trimmed.strip_prefix("- ").unwrap_or(trimmed);

        // Try to extract numbered slug and title: "1.2 Some Title"
        if let Some((slug, title)) = parse_numbered_entry(content) {
            let depth = calculate_depth(line);
            pages.push(WikiPageMeta { slug, title, depth });
        }
    }

    pages
}

/// Parse a numbered entry like "1.2 Some Title" into (slug, title).
fn parse_numbered_entry(s: &str) -> Option<(String, String)> {
    let re = numbered_entry_regex();
    let caps = re.captures(s)?;
    let number = caps.get(1)?.as_str();
    let title = caps.get(2)?.as_str().trim();
    let slug = format!("{}-{}", number, slugify(title));
    Some((slug, title.to_string()))
}

/// Calculate depth based on leading whitespace (each 2 spaces = 1 level).
fn calculate_depth(line: &str) -> usize {
    let leading_spaces = line.len() - line.trim_start().len();
    leading_spaces / 2
}

/// Split the combined content from `read_wiki_contents` into individual pages.
///
/// Content is separated by `# Page: <title>` lines.
pub fn split_pages(
    content: &str,
    structure: &[WikiPageMeta],
) -> Vec<crate::types::WikiPage> {
    let re = page_separator_regex();

    let mut pages = Vec::new();
    let mut matches: Vec<(usize, String)> = Vec::new();

    for cap in re.captures_iter(content) {
        let full_match = cap.get(0).unwrap();
        let title = cap.get(1).unwrap().as_str().to_string();
        matches.push((full_match.start(), title));
    }

    for (i, (start, title)) in matches.iter().enumerate() {
        // Content starts after the "# Page: ..." line
        let content_start = content[*start..].find('\n').map(|pos| start + pos + 1).unwrap_or(content.len());
        let content_end = if i + 1 < matches.len() {
            matches[i + 1].0
        } else {
            content.len()
        };

        let page_content = content[content_start..content_end].trim().to_string();

        // Try to find matching structure entry by title
        let meta = structure.iter().find(|m| m.title == *title);

        let (slug, depth) = if let Some(m) = meta {
            (m.slug.clone(), m.depth)
        } else {
            // Fallback: generate slug from title
            let slug = slugify(title);
            (slug, 0)
        };

        pages.push(crate::types::WikiPage {
            slug,
            title: title.clone(),
            depth,
            content: page_content,
        });
    }

    // Cross-validate count
    if !structure.is_empty() && pages.len() != structure.len() {
        eprintln!(
            "Warning: expected {} pages from structure, but split {} pages from content",
            structure.len(),
            pages.len()
        );
    }

    pages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_structure_basic() {
        let text = "\
Available pages for anthropics/claude-code:

- 1 Overview
  - 1.1 System Architecture
  - 1.2 Feature Evolution
- 2 User Guide
  - 2.1 Installation";

        let pages = parse_wiki_structure(text);
        assert_eq!(pages.len(), 5);
        assert_eq!(pages[0].slug, "1-overview");
        assert_eq!(pages[0].title, "Overview");
        assert_eq!(pages[0].depth, 0);
        assert_eq!(pages[1].slug, "1.1-system-architecture");
        assert_eq!(pages[1].title, "System Architecture");
        assert_eq!(pages[1].depth, 1);
        assert_eq!(pages[3].slug, "2-user-guide");
        assert_eq!(pages[3].title, "User Guide");
        assert_eq!(pages[3].depth, 0);
    }

    #[test]
    fn test_parse_structure_empty() {
        let pages = parse_wiki_structure("");
        assert!(pages.is_empty());
    }

    #[test]
    fn test_parse_structure_no_header() {
        let text = "- 1 Overview\n  - 1.1 Details";
        let pages = parse_wiki_structure(text);
        assert_eq!(pages.len(), 2);
    }

    #[test]
    fn test_split_pages_basic() {
        let content = "\
# Page: Overview

This is the overview.

# Page: Architecture

This is the architecture.
";
        let structure = vec![
            WikiPageMeta {
                slug: "1-overview".to_string(),
                title: "Overview".to_string(),
                depth: 0,
            },
            WikiPageMeta {
                slug: "1.1-architecture".to_string(),
                title: "Architecture".to_string(),
                depth: 1,
            },
        ];

        let pages = split_pages(content, &structure);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].slug, "1-overview");
        assert_eq!(pages[0].title, "Overview");
        assert!(pages[0].content.contains("This is the overview."));
        assert_eq!(pages[1].slug, "1.1-architecture");
        assert!(pages[1].content.contains("This is the architecture."));
    }

    #[test]
    fn test_split_pages_with_leading_content() {
        let content = "\
Some preamble text that comes before the first page.

# Page: Overview

The overview content.
";
        let structure = vec![WikiPageMeta {
            slug: "1-overview".to_string(),
            title: "Overview".to_string(),
            depth: 0,
        }];

        let pages = split_pages(content, &structure);
        assert_eq!(pages.len(), 1);
        assert!(pages[0].content.contains("The overview content."));
    }

    #[test]
    fn test_split_pages_special_chars_in_title() {
        let content = "# Page: C++ & Rust: A Comparison\n\nContent here.\n";
        let structure = vec![];

        let pages = split_pages(content, &structure);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].title, "C++ & Rust: A Comparison");
    }

    #[test]
    fn test_split_pages_crlf() {
        let content = "# Page: Overview\r\n\r\nContent.\r\n";
        let structure = vec![];

        let pages = split_pages(content, &structure);
        assert_eq!(pages.len(), 1);
        assert_eq!(pages[0].title, "Overview");
    }

    #[test]
    fn test_split_pages_count_mismatch_warning() {
        let content = "# Page: Overview\n\nContent.\n";
        let structure = vec![
            WikiPageMeta {
                slug: "1-overview".to_string(),
                title: "Overview".to_string(),
                depth: 0,
            },
            WikiPageMeta {
                slug: "2-missing".to_string(),
                title: "Missing".to_string(),
                depth: 0,
            },
        ];

        // Should still return the 1 page it found, with a warning to stderr
        let pages = split_pages(content, &structure);
        assert_eq!(pages.len(), 1);
    }
}
