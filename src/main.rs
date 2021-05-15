extern crate time;

use std::process;

use clap::{App, Arg, ArgMatches};
use console::{Emoji, Term};
use log::{debug, error, LevelFilter};
use once_cell::sync::OnceCell;
use simplelog::{ConfigBuilder, TerminalMode};

use scoring::scorable::Scorable;

use crate::github::client::pool::{GitHubConnectionPool, GitHubConnectionPoolManager};
use crate::github::utils::analyzer::AnalyzerBuilder;
use crate::github::utils::pull_request_data::PullRequestData;
use crate::scoring::score::{Score, ScoreType};

#[path = "error.rs"]
mod prolice_error;

#[path = "metadata.rs"]
mod prolice_metadata;

mod github;

mod scoring;

mod report;

// CLI params ---
const GITHUB_TOKEN_PARAM: &str = "github-token";
const LOG_LEVEL_PARAM: &str = "log-level";
const OWNER_PARAM: &str = "owner";
const PR_NUMBER_PARAM: &str = "pr-number";
const REPOSITORY_PARAM: &str = "repository";
const SAMPLE_SIZE_PARAM: &str = "sample-size";

// CLI flags ---
const INCLUDE_MERGE_PRS_FLAG: &str = "include-merge-prs";
const PRINT_LEGENDS_FLAG: &str = "print-legends";
const SILENT_MODE_FLAG: &str = "silent-mode";

// Default values ---
const DEFAULT_SAMPLE_SIZE: u8 = 100;
const MAX_SAMPLE_SIZE: u8 = DEFAULT_SAMPLE_SIZE;
const MIN_SAMPLE_SIZE: u8 = 1;

const DEFAULT_CONNECTION_POOL_SIZE: u8 = DEFAULT_SAMPLE_SIZE;
/* Using bigger pools than this default usually triggers *more* API abuse detection mechanisms from GitHub
* ('more' because GitHub's definition of 'abuse' is arbitrary; sometimes a pool of 300+ concurrent connections
* may trigger an abuse alarm in some requests, other times all of them will pass without hiccups).
*
* We usually skip blocked requests if GitHub gets too trigger happy with its abuse heuristics, but an
* incomplete PR, even partially incomplete, is completely discarded; which ultimately shrinks our analysis pool
* (which we don't want).
*
* So it's overall better to use rational defaults and try that as many concurrent connections as possible
* get completed successfully, than have a massive pool where half of the requests fail (it may get 'faster'
* results, but the quality of the analysis is going to be substantially worse is half of the analysis
* pool gets discarded for being incomplete).
*
* https://docs.github.com/en/rest/guides/best-practices-for-integrators#dealing-with-abuse-rate-limits
*/

/// Global connection pool for GitHub.
/// <br><br>
/// We need the connection pool to be static (and thus have a static lifetime) because all GitHub calls
/// are asynchronous. This allows us to perform hundreds of calls simultaneously, but with one caveat:
/// spawned async tasks require complete ownership of the involved data for indefinite duration, and thus
/// borrowed data (and this involves the borrowed shared-amongst-all connection pool) must have a 'static
/// lifetime (so as to guarantee it does not get dereferenced along the way).
/// We cannot set this 'static lifetime inside the main function (which is technically a tokio wrapper
/// itself) either, so this is the most elegant way to go around the problem.
/// https://stackoverflow.com/a/27826181
static GITHUB_CONNECTION_POOL: OnceCell<GitHubConnectionPool> = OnceCell::new();

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // initialize CLI access ---
    let args = setup_cli();

    // determine if console is user attended or not (ie: output is being piped into a file) ---
    let console_is_user_attended = console::user_attended();

    // parse obligatory params ---

    // since the clamp crate is in charge of making sure these obligatory params are fulfilled as requirements,
    // any of these unwrap(s) ending in error should be theoretically impossible under normal circumstances.
    // still, it doesn't hurt to do a quick validation, just in case

    let github_token = args.value_of(GITHUB_TOKEN_PARAM).unwrap_or_else(|| {
        eprintln!("{} is an obligatory param! Aborting operation.", GITHUB_TOKEN_PARAM);
        process::exit(1)
    });

    let owner = args.value_of(OWNER_PARAM).unwrap_or_else(|| {
        eprintln!("{} is an obligatory param! Aborting operation.", OWNER_PARAM);
        process::exit(1)
    });

    let repository = args.value_of(REPOSITORY_PARAM).unwrap_or_else(|| {
        eprintln!("{} is an obligatory param! Aborting operation.", REPOSITORY_PARAM);
        process::exit(1)
    });

    let sample_size: u8 = args.value_of_t_or_exit(SAMPLE_SIZE_PARAM);

    // parse optional params & flags ---
    let silent_mode: bool = !console_is_user_attended || args.is_present(SILENT_MODE_FLAG);

    let include_merge_prs: bool = args.is_present(INCLUDE_MERGE_PRS_FLAG);

    let print_metric_legends: bool = !silent_mode && args.is_present(PRINT_LEGENDS_FLAG);

    let selected_pr_number: Result<u64, _> = args.value_of_t(PR_NUMBER_PARAM);

    // initialize logging facade ---
    let log_level = if !silent_mode {
        // if console _is_ attended, honor selected log-level
        args.value_of_t_or_exit(LOG_LEVEL_PARAM)
    } else {
        // automatically turn off all logs if console is unattended
        // (specially useful for piping results to a file without the extra 'noise')
        LevelFilter::Off
    };

    init_logging(log_level);

    // initialize GitHub's connection pool ---
    GITHUB_CONNECTION_POOL.set(
        GitHubConnectionPool::new(
            GitHubConnectionPoolManager::new(github_token),
            DEFAULT_CONNECTION_POOL_SIZE as usize // (must be a good API citizen and use a rational number of concurrent connections, or risk rejection by remote endpoint)
        )
    ).unwrap_or_else(|e| {
        error!("Could not initialize GitHub's connection pool. This is a mandatory requirement for operation. Aborting immediately.");
        panic!(e) // this is a fatal error that involves delving into the codebase; ungracefully panic
    });

    let github_connection_pool = GITHUB_CONNECTION_POOL.get().unwrap(); // we just initialized it above, no need to error check (again)

    // initialize app ---
    let stdout: Option<Term> = if !silent_mode {
        Some(Term::stdout())
    } else {
        None
    };

    if let Some(stdout) = &stdout {
        stdout.write_line(get_logo())?;

        let paper_emoji = Emoji("📃", "*");
        let looking_glass_emoji = Emoji("🔍", "*");
        let number_emoji = Emoji("🔢", "*");
        let ruler_emoji = Emoji("📏", "*");

        stdout.write_line(&format!("{} Initializing analysis for [{}].", paper_emoji, owner))?;
        stdout.write_line(&format!("{} Target is [{}].", looking_glass_emoji, repository))?;

        if let Ok(pr_number) = selected_pr_number {
            stdout
                .write_line(&format!("{} Selected PR number is [{}].", number_emoji, pr_number))?;
        } else {
            stdout.write_line(&format!(
                "{} Using a sample size of [{}] PRs per repository.",
                ruler_emoji, sample_size
            ))?;
        }

        stdout.write_line(&"=".repeat(stdout.size().1 as usize))?; // print separator for whole length of stdout
    }

    // initialize repo/pr analyzer ---
    let analyzer = AnalyzerBuilder::new(owner, repository, github_token, github_connection_pool)
        .init()
        .await
        .unwrap_or_else(|e| {
            error!(
                "There was an error initializing Analyzer for [{}]/[{}]. Aborting operation.",
                owner, repository
            );
            error!("{}", e);
            // we don't to panic in this potentially expected scenario (owner or repo name(s) may be misspelled in passed args)
            // exit gracefully, but with an error
            process::exit(1)
        });

    // execute analysis for selected target(s) ---
    let result_out = Term::stdout(); // result always ignores 'silent' flag

    if let Ok(pr_number) = selected_pr_number {
        // https://github.com/warnerbrostv/Project-Brainiac-Java/pull/5486
        let pr_score: Score = analyzer
            .retrieve_pr_data(pr_number) // 6909/6913 for attachments; 5486 for extensive commentary; 6854 for a REALLY LONG wip PR; 6830 for more deletions than additions
            .await
            .unwrap_or_else(|e| {
                error!("{}", e);
                process::exit(1);
            })
            .get_score();

        print_metrics_legends(print_metric_legends, &result_out); // print metrics' legends, if flag allows for it
        result_out.write_line(&format!("{}", pr_score))?;
    } else {
        let repo_score: Score = analyzer
            .retrieve_repo_data(sample_size)
            .await
            .iter()
            .filter_map(|pull_request_data_result| pull_request_data_result.as_ref().ok())
            .filter(|pull_request_data| {
                let passes_filter = include_merge_prs || !pull_request_data.is_merge_pr();

                if !passes_filter {
                    debug!(
                        "[{}]/[{}] filtered out for being a merge PR.",
                        repository,
                        pull_request_data.pr_number()
                    )
                }

                passes_filter
            })
            .collect::<Vec<&PullRequestData>>()
            .get_score();

        print_metrics_legends(print_metric_legends, &result_out); // print metrics' legends, if flag allows for it
        result_out.write_line(&format!("{}", repo_score))?;
    }

    Ok(())
}

/// Retrieves the application's ASCII-art logo.
fn get_logo() -> &'static str {
    r#"
        ooooooooo.   ooooooooo.             oooo   o8o
        `888   `Y88. `888   `Y88.           `888   `"'
         888   .d88'  888   .d88'  .ooooo.   888  oooo   .ooooo.   .ooooo.
         888ooo88P'   888ooo88P'  d88' `88b  888  `888  d88' `"Y8 d88' `88b
         888          888`88b.    888   888  888   888  888       888ooo888
         888          888  `88b.  888   888  888   888  888   .o8 888    .o
        o888o        o888o  o888o `Y8bod8P' o888o o888o `Y8bod8P' `Y8bod8P'
        ------------ What you gonna do when they come for you -------------
    "#
}

/// Prints analyzed metrics' legends into target [`Term`], if `toggle` is `true`;
fn print_metrics_legends(toggle: bool, term: &Term) {
    if !toggle {
        return;
    }

    term.write_line(&ScoreType::get_legends()).unwrap_or_else(|e| {
        error!("An error has occurred while printing metrics' legends to term! Error = {}", e);
    });
    term.write_line(&"=".repeat(term.size().1 as usize)).unwrap_or_else(|e| {
        error!("An error has occurred while printing line separator term! Error = {}", e);
    });
}

/// Initializes the `Log` crate's logging facade.
fn init_logging(log_level: LevelFilter) {
    simplelog::TermLogger::init(
        log_level,
        ConfigBuilder::new()
            .add_filter_allow_str(prolice_metadata::package_name())
            .set_time_to_local(true)
            .build(),
        TerminalMode::Mixed,
    )
    .unwrap() // we want to panic if the logger couldn't be initialized, so the unwrap() is adequate
}

/// Sets up the CLI for the whole application.
fn setup_cli() -> ArgMatches {
    return App::new(prolice_metadata::package_name())
        .version(prolice_metadata::full_version())
        .author(prolice_metadata::authors())
        .about(prolice_metadata::description())
        // params start here ---
        .arg(
            Arg::new(OWNER_PARAM)
                .long(OWNER_PARAM)
                .short('O')
                .about("The owner of the repository under scrutiny")
                .required(true)
                .takes_value(true)
                .case_insensitive(false),
        )
        .arg(
            Arg::new(REPOSITORY_PARAM)
                .long(REPOSITORY_PARAM)
                .short('R')
                .about("The repository under scrutiny")
                .required(true)
                .takes_value(true)
                .case_insensitive(false),
        )
        .arg(
            Arg::new(SAMPLE_SIZE_PARAM)
                .long(SAMPLE_SIZE_PARAM)
                .short('S')
                .about(
                    "The amount of PRs that will be fetched as sample for the analysis (unless a specific \
                    PR number is selected as individual target)"
                )
                .required(true)
                .takes_value(true)
                .validator(|value| {
                    let value = value.parse::<usize>();

                    if value.is_err() {
                        return Err(format!(
                            "Supplied value must be an integer number between {} and {}",
                            MIN_SAMPLE_SIZE, MAX_SAMPLE_SIZE
                        ));
                    }

                    let value = value.unwrap();

                    if !(1..=100).contains(&value) {
                        return Err(format!(
                            "Supplied value must be an integer number between {} and {}, but was {}",
                            MIN_SAMPLE_SIZE, MAX_SAMPLE_SIZE, value
                        ));
                    }

                    Ok(())
                })
                .default_value(&DEFAULT_SAMPLE_SIZE.to_string())
                .conflicts_with(PR_NUMBER_PARAM) // user must either select sample size or a specific PR; not both
        )
        .arg(
            Arg::new(PR_NUMBER_PARAM)
                .long(PR_NUMBER_PARAM)
                .short('P')
                .about("A specific pull-request to be selected as target for the analysis.")
                .required(false)
                .takes_value(true)
                .validator(|value| {
                    let value = value.parse::<u64>();

                    if value.is_err() {
                        return Err("Supplied value must be an integer number");
                    }

                    Ok(())
                })
                .conflicts_with(SAMPLE_SIZE_PARAM) // user must either select sample size or a specific PR; not both
        )
        .arg(
            Arg::new(GITHUB_TOKEN_PARAM)
                .long(GITHUB_TOKEN_PARAM)
                .short('G')
                .about("Sets the personal access token under which to perform the PR analysis")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::new(LOG_LEVEL_PARAM)
                .long(LOG_LEVEL_PARAM)
                .short('L')
                .about("Overrides the logging verbosity for the whole application")
                .required(false)
                .takes_value(true) // redundant by specifying 'possible_values'; declared here just to keep homogeneous build structure
                .possible_values(&[
                    LevelFilter::Info.as_str(),
                    LevelFilter::Debug.as_str(),
                    LevelFilter::Trace.as_str(),
                    LevelFilter::Warn.as_str(),
                    LevelFilter::Error.as_str(),
                    LevelFilter::Off.as_str(),
                ])
                .case_insensitive(true)
                .default_value(prolice_metadata::default_log_level().as_str())
                .conflicts_with(SILENT_MODE_FLAG),
        )
        // optional flags start here ---
        .arg(
            Arg::new(INCLUDE_MERGE_PRS_FLAG)
                .long(INCLUDE_MERGE_PRS_FLAG)
                .short('m')
                .about(
                    "Marks merge-PRs as valid targets for analysis (by default these are excluded). \
                    Valid only for whole Repository analysis; for individual PR analysis this flag is \
                    ignored"
                )
                .takes_value(false),
        )
        .arg(
            Arg::new(SILENT_MODE_FLAG)
                .long(SILENT_MODE_FLAG)
                .short('s')
                .about(
                    "Marks the operation as silent, which turns off all logging and printing to stdout, \
                    with the sole exception of the analysis results. This makes it useful for piping \
                    just the results, without the added 'noise'. (NOTE: piping is automatically detected, \
                    which activates silent-mode without having to explicitly add the flag to the command)"
                )
                .takes_value(false)
                .conflicts_with(LOG_LEVEL_PARAM)
                .conflicts_with(PRINT_LEGENDS_FLAG),
        )
        .arg(
            Arg::new(PRINT_LEGENDS_FLAG)
                .long(PRINT_LEGENDS_FLAG)
                .short('l')
                .about(
                    "Prints the metrics' legends before sending the operation results to stdout."
                )
                .takes_value(false)
                .conflicts_with(SILENT_MODE_FLAG),
        )
        .get_matches();
}
