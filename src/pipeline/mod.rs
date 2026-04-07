pub mod json;
pub mod markdown;
pub mod mermaid;

use std::path::PathBuf;
use std::time::Duration;

use crate::mcp::{McpClient, McpError};
use crate::types::{OutputMode, RepoId};
use crate::wiki;
use crate::writer::Output;

/// Options for the `pull` command.
pub struct PullOptions {
    pub output: OutputMode,
    pub pages: Option<Vec<String>>,
    pub exclude: Option<Vec<String>>,
    pub timeout_connect: Duration,
    pub timeout_read: Duration,
    pub mermaid: Option<String>,
    pub verbose: bool,
}

/// Execute the `pull` command: fetch wiki and produce output.
pub fn pull(
    repo: &RepoId,
    options: &PullOptions,
    endpoint: Option<&str>,
    on_status: &dyn Fn(&str),
) -> Result<Output, McpError> {
    // Validate --mermaid requires -o
    if options.mermaid.is_some() && matches!(options.output, OutputMode::Stdout) {
        return Err(McpError::InvalidArgs {
            message: "--mermaid requires -o to specify output directory or file.".to_string(),
        });
    }

    on_status("Connecting to DeepWiki MCP server...");
    let mut client =
        McpClient::connect(endpoint, options.timeout_connect, options.timeout_read)?;

    on_status("Fetching wiki structure...");
    let structure_text = client.read_wiki_structure(&repo.to_string())?;
    let structure = wiki::parse_wiki_structure(&structure_text);

    if structure.is_empty() {
        return Err(McpError::RepoNotFound {
            repo: repo.to_string(),
        });
    }

    on_status("Fetching wiki contents...");
    let contents_text = client.read_wiki_contents(&repo.to_string())?;
    let pages = wiki::split_pages(&contents_text, &structure);

    // Apply filters
    let pages = wiki::filter::filter_pages(
        pages,
        options.pages.as_deref(),
        options.exclude.as_deref(),
    );

    if pages.is_empty() {
        return Err(McpError::InvalidArgs {
            message: "No pages remaining after filtering. Check --pages/--exclude values."
                .to_string(),
        });
    }

    on_status(&format!("Processing {} pages...", pages.len()));

    // Format output based on mode
    let output = match &options.output {
        OutputMode::Stdout => {
            let content = markdown::format_stdout(&pages);
            Output::Stdout(content)
        }
        OutputMode::SingleFile(path) => {
            let content = markdown::format_single_file(&pages);
            Output::SingleFile {
                path: path.clone(),
                content,
            }
        }
        OutputMode::Directory(dir) => {
            let mut files = markdown::format_directory(&pages, &repo.owner, &repo.repo);

            // Apply mermaid rendering if requested
            if let Some(ref format) = options.mermaid {
                on_status("Rendering mermaid diagrams...");
                for (path, content) in files.iter_mut() {
                    let slug = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown");
                    match mermaid::render_mermaid_in_content(content, format, dir, slug) {
                        Ok((new_content, _)) => *content = new_content,
                        Err(e) => eprintln!("Warning: mermaid rendering failed for {}: {e}", path.display()),
                    }
                }
            }

            Output::Directory {
                base_dir: dir.clone(),
                files,
            }
        }
    };

    Ok(output)
}

/// Options for the `list` command.
pub struct ListOptions {
    pub json: bool,
    pub timeout_connect: Duration,
    pub timeout_read: Duration,
    pub verbose: bool,
}

/// Execute the `list` command: fetch structure and display.
pub fn list(
    repo: &RepoId,
    options: &ListOptions,
    endpoint: Option<&str>,
    on_status: &dyn Fn(&str),
) -> Result<String, McpError> {
    on_status("Connecting to DeepWiki MCP server...");
    let mut client =
        McpClient::connect(endpoint, options.timeout_connect, options.timeout_read)?;

    on_status("Fetching wiki structure...");
    let structure_text = client.read_wiki_structure(&repo.to_string())?;
    let structure = wiki::parse_wiki_structure(&structure_text);

    if structure.is_empty() {
        return Err(McpError::RepoNotFound {
            repo: repo.to_string(),
        });
    }

    let output = if options.json {
        json::format_json_list(&repo.to_string(), &structure)
    } else {
        json::format_text_list(&repo.to_string(), &structure)
    };

    Ok(output)
}

/// Determine output mode from the -o argument.
pub fn resolve_output_mode(output_arg: Option<&str>, repo: &RepoId) -> OutputMode {
    match output_arg {
        None => OutputMode::Stdout,
        Some(path) => {
            let p = PathBuf::from(path);
            // If path ends with / or \ or is an existing directory → directory mode
            if path.ends_with('/') || path.ends_with('\\') || p.is_dir() {
                let dir = if p.file_name().is_some()
                    && !p.to_string_lossy().ends_with('/')
                    && !p.to_string_lossy().ends_with('\\')
                {
                    p
                } else {
                    p.join(format!("{}-{}", repo.owner, repo.repo))
                };
                OutputMode::Directory(dir)
            } else {
                OutputMode::SingleFile(p)
            }
        }
    }
}
