use std::process;
use std::time::Duration;

use clap::{Parser, Subcommand};
use deepwiki_dl::types::RepoId;
use deepwiki_dl::{list, pull, resolve_output_mode, write_output, ListOptions, PullOptions};
use indicatif::{ProgressBar, ProgressStyle};

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
        #[arg(value_parser = parse_repo_id)]
        repo: RepoId,
        #[arg(short, long)]
        output: Option<String>,
        #[arg(long, value_name = "FORMAT")]
        mermaid: Option<String>,
        #[arg(short, long, value_delimiter = ',')]
        pages: Option<Vec<String>>,
        #[arg(short = 'x', long, value_delimiter = ',')]
        exclude: Option<Vec<String>>,
        #[arg(short, long, default_value = "30")]
        timeout: u64,
        #[arg(short, long)]
        verbose: bool,
        #[arg(short, long)]
        quiet: bool,
        #[arg(long)]
        no_color: bool,
    },
    /// List available sections for a repository
    List {
        #[arg(value_parser = parse_repo_id)]
        repo: RepoId,
        #[arg(long)]
        json: bool,
        #[arg(short, long, default_value = "30")]
        timeout: u64,
        #[arg(short, long)]
        verbose: bool,
        #[arg(short, long)]
        quiet: bool,
        #[arg(long)]
        no_color: bool,
    },
}

fn parse_repo_id(s: &str) -> Result<RepoId, String> {
    s.parse()
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
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
        }) => run_pull(repo, output, mermaid, pages, exclude, timeout, verbose, quiet, no_color),

        Some(Commands::List {
            repo,
            json,
            timeout,
            verbose,
            quiet,
            no_color,
        }) => run_list(repo, json, timeout, verbose, quiet, no_color),

        None => {
            if let Some(repo) = cli.repo {
                run_pull(
                    repo,
                    cli.output,
                    cli.mermaid,
                    cli.pages,
                    cli.exclude,
                    cli.timeout,
                    cli.verbose,
                    cli.quiet,
                    cli.no_color,
                )
            } else {
                Cli::parse_from(["deepwiki-dl", "--help"]);
                Ok(())
            }
        }
    };

    if let Err(e) = result {
        print_error(&e);
        process::exit(1);
    }
}

fn print_error(err: &Box<dyn std::error::Error>) {
    let err_msg = err.to_string();
    eprintln!("{} {}", yansi::Paint::red("Error:"), &err_msg);
    if err_msg.contains("not indexed") {
        eprintln!(
            "\n{} Visit {} to add this repository.",
            yansi::Paint::yellow("Hint:"),
            "https://deepwiki.com"
        );
    } else if err_msg.contains("too large") {
        eprintln!(
            "\n{} Use --pages to fetch specific sections.",
            yansi::Paint::yellow("Hint:"),
        );
    } else if err_msg.contains("--mermaid requires") {
        eprintln!(
            "\n{} Example: deepwiki-dl repo -o ./docs/ --mermaid svg",
            yansi::Paint::yellow("Hint:"),
        );
    }
}

fn make_spinner(quiet: bool) -> ProgressBar {
    if quiet {
        return ProgressBar::hidden();
    }
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .template("{spinner:.cyan} {msg}")
            .unwrap(),
    );
    spinner.enable_steady_tick(Duration::from_millis(80));
    spinner
}

fn run_pull(
    repo: RepoId,
    output: Option<String>,
    mermaid: Option<String>,
    pages: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    timeout: u64,
    verbose: bool,
    quiet: bool,
    no_color: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if no_color {
        yansi::disable();
    }

    let endpoint = std::env::var("DEEPWIKI_DL_MCP_ENDPOINT").ok();
    let output_mode = resolve_output_mode(output.as_deref(), &repo);

    let spinner = make_spinner(quiet);
    let status_fn = move |msg: &str| {
        spinner.set_message(msg.to_string());
    };

    let options = PullOptions {
        output: output_mode,
        pages,
        exclude,
        timeout_connect: Duration::from_secs(timeout),
        timeout_read: Duration::from_secs(timeout * 4),
        mermaid,
        verbose,
    };

    let output = pull(&repo, &options, endpoint.as_deref(), &status_fn)?;

    // Finish spinner before writing to stdout
    status_fn("Writing output...");

    let result = write_output(output)?;

    if !quiet && result.files_written > 0 {
        let msg = format!(
            "Done! Wrote {} file(s) ({})",
            result.files_written, result.mode
        );
        eprintln!("{}", yansi::Paint::green(&msg));
    }

    Ok(())
}

fn run_list(
    repo: RepoId,
    json: bool,
    timeout: u64,
    verbose: bool,
    quiet: bool,
    no_color: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    if no_color {
        yansi::disable();
    }

    let endpoint = std::env::var("DEEPWIKI_DL_MCP_ENDPOINT").ok();

    let spinner = make_spinner(quiet);
    let status_fn = move |msg: &str| {
        spinner.set_message(msg.to_string());
    };

    let options = ListOptions {
        json,
        timeout_connect: Duration::from_secs(timeout),
        timeout_read: Duration::from_secs(timeout * 4),
        verbose,
    };

    let output = list(&repo, &options, endpoint.as_deref(), &status_fn)?;

    println!("{output}");

    Ok(())
}
