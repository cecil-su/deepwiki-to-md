use anyhow::Result;
use clap::{Parser, Subcommand};
use deepwiki_dl::types::RepoId;

#[derive(Parser)]
#[command(
    name = "deepwiki-dl",
    version,
    about = "Download DeepWiki documentation to local Markdown files"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Repository in owner/repo format or URL (used when no subcommand)
    #[arg(value_parser = parse_repo_id)]
    repo: Option<RepoId>,

    /// Output directory or file path (auto-detects mode)
    #[arg(short, long)]
    output: Option<String>,

    /// Render mermaid diagrams to svg or png (requires -o)
    #[arg(long, value_name = "FORMAT")]
    mermaid: Option<String>,

    /// Only fetch specific sections (comma-separated slugs)
    #[arg(short, long, value_delimiter = ',')]
    pages: Option<Vec<String>>,

    /// Exclude specific sections (comma-separated slugs)
    #[arg(short = 'x', long, value_delimiter = ',')]
    exclude: Option<Vec<String>>,

    /// Request timeout in seconds
    #[arg(short, long, default_value = "30")]
    timeout: u64,

    /// Show detailed logs
    #[arg(short, long)]
    verbose: bool,

    /// Only output errors
    #[arg(short, long)]
    quiet: bool,

    /// Disable colored output
    #[arg(long)]
    no_color: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Pull documentation from DeepWiki (default command)
    Pull {
        /// Repository in owner/repo format or URL
        #[arg(value_parser = parse_repo_id)]
        repo: RepoId,

        /// Output directory or file path
        #[arg(short, long)]
        output: Option<String>,

        /// Render mermaid diagrams to svg or png (requires -o)
        #[arg(long, value_name = "FORMAT")]
        mermaid: Option<String>,

        /// Only fetch specific sections (comma-separated slugs)
        #[arg(short, long, value_delimiter = ',')]
        pages: Option<Vec<String>>,

        /// Exclude specific sections (comma-separated slugs)
        #[arg(short = 'x', long, value_delimiter = ',')]
        exclude: Option<Vec<String>>,

        /// Request timeout in seconds
        #[arg(short, long, default_value = "30")]
        timeout: u64,

        /// Show detailed logs
        #[arg(short, long)]
        verbose: bool,

        /// Only output errors
        #[arg(short, long)]
        quiet: bool,

        /// Disable colored output
        #[arg(long)]
        no_color: bool,
    },
    /// List available sections for a repository
    List {
        /// Repository in owner/repo format or URL
        #[arg(value_parser = parse_repo_id)]
        repo: RepoId,

        /// Output as JSON
        #[arg(long)]
        json: bool,

        /// Request timeout in seconds
        #[arg(short, long, default_value = "30")]
        timeout: u64,

        /// Show detailed logs
        #[arg(short, long)]
        verbose: bool,

        /// Only output errors
        #[arg(short, long)]
        quiet: bool,

        /// Disable colored output
        #[arg(long)]
        no_color: bool,
    },
}

fn parse_repo_id(s: &str) -> Result<RepoId, String> {
    s.parse()
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Resolve command: if no subcommand, treat positional repo as `pull`
    match cli.command {
        Some(Commands::Pull {
            repo,
            output,
            mermaid,
            pages,
            exclude,
            timeout,
            verbose,
            quiet,
            no_color,
        }) => {
            if no_color {
                yansi::disable();
            }
            eprintln!("[pull] repo={repo}, output={output:?}, mermaid={mermaid:?}, pages={pages:?}, exclude={exclude:?}, timeout={timeout}, verbose={verbose}, quiet={quiet}");
        }
        Some(Commands::List {
            repo,
            json,
            timeout,
            verbose,
            quiet,
            no_color,
        }) => {
            if no_color {
                yansi::disable();
            }
            eprintln!("[list] repo={repo}, json={json}, timeout={timeout}, verbose={verbose}, quiet={quiet}");
        }
        None => {
            // Default command: pull with top-level args
            if let Some(repo) = cli.repo {
                if cli.no_color {
                    yansi::disable();
                }
                eprintln!("[pull] repo={repo}, output={:?}, mermaid={:?}, pages={:?}, exclude={:?}, timeout={}, verbose={}, quiet={}", cli.output, cli.mermaid, cli.pages, cli.exclude, cli.timeout, cli.verbose, cli.quiet);
            } else {
                // No repo provided — show help
                Cli::parse_from(["deepwiki-dl", "--help"]);
            }
        }
    }

    Ok(())
}
