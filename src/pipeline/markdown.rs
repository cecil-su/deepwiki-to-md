use std::collections::HashSet;
use std::path::PathBuf;

use regex::Regex;

use crate::types::WikiPage;

/// Sanitize a slug for use as a filename. Removes characters invalid on Windows/Linux/macOS.
pub fn sanitize_filename(slug: &str) -> String {
    let invalid_chars = ['/', '\\', ':', '?', '*', '"', '<', '>', '|'];
    slug.chars()
        .map(|c| if invalid_chars.contains(&c) { '-' } else { c })
        .collect()
}

/// Rewrite internal DeepWiki links to local relative paths.
///
/// Converts `https://deepwiki.com/{owner}/{repo}/{slug}` to `./{slug}.md`
/// only if the slug is in the known set. Uses a pre-compiled regex.
pub fn rewrite_internal_links(
    content: &str,
    re: &Regex,
    known_slugs: &HashSet<String>,
) -> String {
    re.replace_all(content, |caps: &regex::Captures| {
        let slug = caps.get(1).unwrap().as_str();
        if known_slugs.contains(slug) {
            format!("(./{}.md)", sanitize_filename(slug))
        } else {
            caps.get(0).unwrap().as_str().to_string()
        }
    })
    .to_string()
}

/// Format pages for directory output mode.
///
/// Returns a list of (relative_path, content) pairs.
pub fn format_directory(
    pages: &[WikiPage],
    owner: &str,
    repo: &str,
) -> Vec<(PathBuf, String)> {
    let known_slugs: HashSet<String> = pages.iter().map(|p| p.slug.clone()).collect();

    // Pre-compile regex once for all pages
    let pattern = format!(
        r"\(https?://deepwiki\.com/{}/{}/([^)\s]+)\)",
        regex::escape(owner),
        regex::escape(repo)
    );
    let re = Regex::new(&pattern).unwrap();

    pages
        .iter()
        .map(|page| {
            let filename = format!("{}.md", sanitize_filename(&page.slug));
            let content = rewrite_internal_links(&page.content, &re, &known_slugs);
            (PathBuf::from(filename), content)
        })
        .collect()
}

/// Format pages for single-file output mode.
///
/// Uses `<<< SECTION: Title [slug] >>>` separators (dw2md compatible).
pub fn format_single_file(pages: &[WikiPage]) -> String {
    pages
        .iter()
        .map(|page| {
            format!(
                "<<< SECTION: {} [{}] >>>\n\n{}",
                page.title, page.slug, page.content
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Format pages for stdout output (no separators, just concatenated).
pub fn format_stdout(pages: &[WikiPage]) -> String {
    pages
        .iter()
        .map(|page| page.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_clean() {
        assert_eq!(sanitize_filename("1.1-overview"), "1.1-overview");
    }

    #[test]
    fn test_sanitize_filename_special_chars() {
        assert_eq!(sanitize_filename("a/b\\c:d?e"), "a-b-c-d-e");
    }

    fn make_link_regex(owner: &str, repo: &str) -> Regex {
        let pattern = format!(
            r"\(https?://deepwiki\.com/{}/{}/([^)\s]+)\)",
            regex::escape(owner),
            regex::escape(repo)
        );
        Regex::new(&pattern).unwrap()
    }

    #[test]
    fn test_rewrite_links_known_slug() {
        let content = "See [architecture](https://deepwiki.com/owner/repo/1.1-arch) for details.";
        let re = make_link_regex("owner", "repo");
        let slugs: HashSet<String> = ["1.1-arch".to_string()].into();
        let result = rewrite_internal_links(content, &re, &slugs);
        assert_eq!(
            result,
            "See [architecture](./1.1-arch.md) for details."
        );
    }

    #[test]
    fn test_rewrite_links_unknown_slug() {
        let content = "See [external](https://deepwiki.com/owner/repo/unknown-page) for details.";
        let re = make_link_regex("owner", "repo");
        let slugs: HashSet<String> = HashSet::new();
        let result = rewrite_internal_links(content, &re, &slugs);
        assert_eq!(result, content);
    }

    #[test]
    fn test_rewrite_links_mixed() {
        let content = "[a](https://deepwiki.com/o/r/known) and [b](https://deepwiki.com/o/r/unknown)";
        let re = make_link_regex("o", "r");
        let slugs: HashSet<String> = ["known".to_string()].into();
        let result = rewrite_internal_links(content, &re, &slugs);
        assert!(result.contains("(./known.md)"));
        assert!(result.contains("(https://deepwiki.com/o/r/unknown)"));
    }

    #[test]
    fn test_rewrite_links_no_links() {
        let content = "Just plain text with no links.";
        let re = make_link_regex("o", "r");
        let slugs: HashSet<String> = HashSet::new();
        let result = rewrite_internal_links(content, &re, &slugs);
        assert_eq!(result, content);
    }

    #[test]
    fn test_format_directory() {
        let pages = vec![
            WikiPage {
                slug: "1-overview".to_string(),
                title: "Overview".to_string(),
                depth: 0,
                content: "Overview content".to_string(),
            },
            WikiPage {
                slug: "1.1-arch".to_string(),
                title: "Architecture".to_string(),
                depth: 1,
                content: "Arch content".to_string(),
            },
        ];

        let files = format_directory(&pages, "owner", "repo");
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].0, PathBuf::from("1-overview.md"));
        assert_eq!(files[1].0, PathBuf::from("1.1-arch.md"));
    }

    #[test]
    fn test_format_single_file() {
        let pages = vec![
            WikiPage {
                slug: "1-overview".to_string(),
                title: "Overview".to_string(),
                depth: 0,
                content: "Content A".to_string(),
            },
            WikiPage {
                slug: "2-guide".to_string(),
                title: "Guide".to_string(),
                depth: 0,
                content: "Content B".to_string(),
            },
        ];

        let output = format_single_file(&pages);
        assert!(output.contains("<<< SECTION: Overview [1-overview] >>>"));
        assert!(output.contains("<<< SECTION: Guide [2-guide] >>>"));
        assert!(output.contains("Content A"));
        assert!(output.contains("Content B"));
    }

    #[test]
    fn test_format_stdout() {
        let pages = vec![
            WikiPage {
                slug: "1-a".to_string(),
                title: "A".to_string(),
                depth: 0,
                content: "AAA".to_string(),
            },
            WikiPage {
                slug: "2-b".to_string(),
                title: "B".to_string(),
                depth: 0,
                content: "BBB".to_string(),
            },
        ];

        let output = format_stdout(&pages);
        assert!(output.contains("AAA"));
        assert!(output.contains("BBB"));
        assert!(output.contains("---"));
    }
}
