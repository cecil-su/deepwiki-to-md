use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use regex::Regex;

/// A mermaid code block found in markdown content.
#[derive(Debug, PartialEq)]
pub struct MermaidBlock {
    /// Full match including the fences
    pub full_match: String,
    /// Just the mermaid code inside the fences
    pub code: String,
}

/// Extract all mermaid code blocks from markdown content.
pub fn extract_mermaid_blocks(content: &str) -> Vec<MermaidBlock> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?ms)^```mermaid\s*\n(.*?)^```").unwrap());
    re.captures_iter(content)
        .map(|cap| MermaidBlock {
            full_match: cap.get(0).unwrap().as_str().to_string(),
            code: cap.get(1).unwrap().as_str().to_string(),
        })
        .collect()
}

/// Check if mmdc (mermaid-cli) is available on the system (cached).
pub fn is_mmdc_available() -> bool {
    static AVAILABLE: OnceLock<bool> = OnceLock::new();
    *AVAILABLE.get_or_init(|| {
        Command::new("mmdc")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    })
}

/// Render mermaid blocks in content, replacing code blocks with image references.
///
/// Returns the modified content and a list of generated files.
pub fn render_mermaid_in_content(
    content: &str,
    format: &str,
    assets_dir: &Path,
    slug: &str,
) -> Result<(String, Vec<PathBuf>), String> {
    let blocks = extract_mermaid_blocks(content);
    if blocks.is_empty() {
        return Ok((content.to_string(), vec![]));
    }

    if !is_mmdc_available() {
        eprintln!(
            "Warning: mmdc (mermaid-cli) not found. Mermaid diagrams will be kept as code blocks.\n\
             Install with: npm install -g @mermaid-js/mermaid-cli"
        );
        return Ok((content.to_string(), vec![]));
    }

    let mermaid_dir = assets_dir.join("mermaid");
    fs::create_dir_all(&mermaid_dir)
        .map_err(|e| format!("Failed to create mermaid assets dir: {e}"))?;

    let mut result = content.to_string();
    let mut generated_files = Vec::new();

    for (i, block) in blocks.iter().enumerate() {
        let filename = format!("{}-{:03}.{}", slug, i + 1, format);
        let output_path = mermaid_dir.join(&filename);

        // Write mermaid code to temp file
        let temp_input = mermaid_dir.join(format!("_temp_{slug}_{i}.mmd"));
        fs::write(&temp_input, &block.code)
            .map_err(|e| format!("Failed to write temp mermaid file: {e}"))?;

        // Run mmdc
        let status = Command::new("mmdc")
            .arg("-i")
            .arg(&temp_input)
            .arg("-o")
            .arg(&output_path)
            .arg("-e")
            .arg(format)
            .output();

        // Clean up temp file
        let _ = fs::remove_file(&temp_input);

        match status {
            Ok(output) if output.status.success() => {
                let relative_path = format!("assets/mermaid/{filename}");
                let img_ref = format!("![Mermaid Diagram]({})", relative_path);
                result = result.replace(&block.full_match, &img_ref);
                generated_files.push(output_path);
            }
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                eprintln!(
                    "Warning: mmdc failed for {slug} diagram {}: {stderr}",
                    i + 1
                );
                // Keep original code block
            }
            Err(e) => {
                eprintln!("Warning: failed to run mmdc for {slug} diagram {}: {e}", i + 1);
            }
        }
    }

    Ok((result, generated_files))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_mermaid_blocks_single() {
        let content = "Some text\n\n```mermaid\ngraph TD;\n  A-->B;\n```\n\nMore text";
        let blocks = extract_mermaid_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].code.contains("graph TD;"));
        assert!(blocks[0].code.contains("A-->B;"));
    }

    #[test]
    fn test_extract_mermaid_blocks_multiple() {
        let content = "```mermaid\ngraph A\n```\n\ntext\n\n```mermaid\ngraph B\n```\n";
        let blocks = extract_mermaid_blocks(content);
        assert_eq!(blocks.len(), 2);
        assert!(blocks[0].code.contains("graph A"));
        assert!(blocks[1].code.contains("graph B"));
    }

    #[test]
    fn test_extract_mermaid_blocks_none() {
        let content = "Just regular markdown\n\n```rust\nfn main() {}\n```\n";
        let blocks = extract_mermaid_blocks(content);
        assert!(blocks.is_empty());
    }

    #[test]
    fn test_extract_mermaid_blocks_multiline() {
        let content = "```mermaid\nsequenceDiagram\n    Alice->>Bob: Hello\n    Bob->>Alice: Hi\n```\n";
        let blocks = extract_mermaid_blocks(content);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].code.contains("sequenceDiagram"));
        assert!(blocks[0].code.contains("Alice->>Bob"));
    }
}
