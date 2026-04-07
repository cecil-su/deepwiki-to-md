use serde::Serialize;

use crate::types::WikiPageMeta;

#[derive(Serialize)]
pub struct JsonListOutput {
    pub repo: String,
    pub pages: Vec<JsonPageEntry>,
}

#[derive(Serialize)]
pub struct JsonPageEntry {
    pub slug: String,
    pub title: String,
    pub depth: usize,
}

/// Format wiki structure as JSON string.
pub fn format_json_list(repo: &str, structure: &[WikiPageMeta]) -> String {
    let output = JsonListOutput {
        repo: repo.to_string(),
        pages: structure
            .iter()
            .map(|m| JsonPageEntry {
                slug: m.slug.clone(),
                title: m.title.clone(),
                depth: m.depth,
            })
            .collect(),
    };

    serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string())
}

/// Format wiki structure as human-readable text.
pub fn format_text_list(repo: &str, structure: &[WikiPageMeta]) -> String {
    let mut output = format!("Pages for {}:\n\n", repo);
    for page in structure {
        let indent = "  ".repeat(page.depth);
        output.push_str(&format!("{}{} {}\n", indent, page.slug, page.title));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_json_list() {
        let structure = vec![
            WikiPageMeta {
                slug: "1-overview".to_string(),
                title: "Overview".to_string(),
                depth: 0,
            },
            WikiPageMeta {
                slug: "1.1-arch".to_string(),
                title: "Architecture".to_string(),
                depth: 1,
            },
        ];

        let json = format_json_list("owner/repo", &structure);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["repo"], "owner/repo");
        assert_eq!(parsed["pages"].as_array().unwrap().len(), 2);
        assert_eq!(parsed["pages"][0]["slug"], "1-overview");
        assert_eq!(parsed["pages"][1]["depth"], 1);
    }

    #[test]
    fn test_format_text_list() {
        let structure = vec![
            WikiPageMeta {
                slug: "1-overview".to_string(),
                title: "Overview".to_string(),
                depth: 0,
            },
            WikiPageMeta {
                slug: "1.1-arch".to_string(),
                title: "Architecture".to_string(),
                depth: 1,
            },
        ];

        let text = format_text_list("owner/repo", &structure);
        assert!(text.contains("Pages for owner/repo:"));
        assert!(text.contains("1-overview Overview"));
        assert!(text.contains("  1.1-arch Architecture"));
    }
}
