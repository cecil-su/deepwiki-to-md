use std::fmt;
use std::str::FromStr;

/// Repository identifier (owner/repo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoId {
    pub owner: String,
    pub repo: String,
}

impl fmt::Display for RepoId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.repo)
    }
}

impl FromStr for RepoId {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();

        // Try to parse as URL first
        if s.starts_with("http://") || s.starts_with("https://") {
            return Self::from_url(s);
        }

        // Try owner/repo format
        Self::from_owner_repo(s)
    }
}

impl RepoId {
    fn from_url(url: &str) -> Result<Self, String> {
        // Remove trailing .git
        let url = url.trim_end_matches(".git");

        // Remove protocol and split by /
        let path = url
            .strip_prefix("https://")
            .or_else(|| url.strip_prefix("http://"))
            .ok_or_else(|| format!("Invalid URL: {url}"))?;

        // Split host and path
        let mut parts = path.splitn(2, '/');
        let _host = parts.next().ok_or_else(|| format!("Invalid URL: {url}"))?;
        let path = parts.next().ok_or_else(|| format!("No path in URL: {url}"))?;

        // Extract owner/repo from path (ignore anything after)
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if segments.len() < 2 {
            return Err(format!(
                "Expected owner/repo in URL, got: {path}"
            ));
        }

        Ok(RepoId {
            owner: segments[0].to_string(),
            repo: segments[1].to_string(),
        })
    }

    fn from_owner_repo(s: &str) -> Result<Self, String> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
            return Err(format!(
                "Expected 'owner/repo' or a URL, got: {s}"
            ));
        }

        Ok(RepoId {
            owner: parts[0].to_string(),
            repo: parts[1].to_string(),
        })
    }
}

/// Metadata about a wiki page (from structure endpoint, no content).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WikiPageMeta {
    pub slug: String,
    pub title: String,
    pub depth: usize,
}

/// A wiki page with content.
#[derive(Debug, Clone)]
pub struct WikiPage {
    pub slug: String,
    pub title: String,
    pub depth: usize,
    pub content: String,
}

/// Output mode determined by -o argument.
#[derive(Debug, Clone)]
pub enum OutputMode {
    /// No -o specified: output to stdout
    Stdout,
    /// -o points to a directory (ends with / or is existing dir)
    Directory(std::path::PathBuf),
    /// -o points to a file
    SingleFile(std::path::PathBuf),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_owner_repo() {
        let repo: RepoId = "anthropics/claude-code".parse().unwrap();
        assert_eq!(repo.owner, "anthropics");
        assert_eq!(repo.repo, "claude-code");
    }

    #[test]
    fn test_parse_deepwiki_url() {
        let repo: RepoId = "https://deepwiki.com/anthropics/claude-code".parse().unwrap();
        assert_eq!(repo.owner, "anthropics");
        assert_eq!(repo.repo, "claude-code");
    }

    #[test]
    fn test_parse_deepwiki_url_with_section() {
        let repo: RepoId =
            "https://deepwiki.com/anthropics/claude-code/1-overview".parse().unwrap();
        assert_eq!(repo.owner, "anthropics");
        assert_eq!(repo.repo, "claude-code");
    }

    #[test]
    fn test_parse_github_url() {
        let repo: RepoId = "https://github.com/anthropics/claude-code".parse().unwrap();
        assert_eq!(repo.owner, "anthropics");
        assert_eq!(repo.repo, "claude-code");
    }

    #[test]
    fn test_parse_github_url_with_git_suffix() {
        let repo: RepoId =
            "https://github.com/anthropics/claude-code.git".parse().unwrap();
        assert_eq!(repo.owner, "anthropics");
        assert_eq!(repo.repo, "claude-code");
    }

    #[test]
    fn test_parse_invalid_format() {
        assert!("just-a-name".parse::<RepoId>().is_err());
        assert!("".parse::<RepoId>().is_err());
        assert!("/repo".parse::<RepoId>().is_err());
        assert!("owner/".parse::<RepoId>().is_err());
    }

    #[test]
    fn test_display() {
        let repo = RepoId {
            owner: "anthropics".to_string(),
            repo: "claude-code".to_string(),
        };
        assert_eq!(repo.to_string(), "anthropics/claude-code");
    }
}
