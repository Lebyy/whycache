mod cli;
mod diagnosis;
mod discovery;
mod error;
mod git;
mod model;
mod parser;
mod render;

use std::io::{self, IsTerminal, Write};

use clap::Parser;

use crate::{
    cli::Cli,
    diagnosis::analyze,
    discovery::{Discovery, LoadResult},
    error::Result,
    git::enrich_with_git,
    render::{Format, render},
};

fn main() {
    if let Err(error) = run() {
        eprintln!("whycache: {error}");
        std::process::exit(error.exit_code());
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let format = Format::from_cli(&cli);
    let discovery = Discovery::new(cli.repo.as_deref())?;

    match discovery.load(&cli)? {
        LoadResult::Ready(pair) => {
            let mut report = analyze(*pair, cli.task.as_deref());
            if let Some(task) = cli.task.as_deref().filter(|_| report.tasks.is_empty()) {
                return Err(error::Error::TaskNotFound(task.to_owned()));
            }
            if cli.git {
                enrich_with_git(&discovery.root, &mut report);
            }
            let colored = format == Format::Human && io::stdout().is_terminal() && !cli.no_color;
            let output = render(&report, format, colored)?;
            print_output(&output)?;
        }
        LoadResult::BaselineCaptured(captured) => {
            let output = render::baseline_captured(&captured, format)?;
            print_output(&output)?;
        }
    }

    Ok(())
}

fn print_output(output: &str) -> Result<()> {
    let mut stdout = io::stdout().lock();
    stdout.write_all(output.as_bytes())?;
    if !output.ends_with('\n') {
        stdout.write_all(b"\n")?;
    }
    Ok(())
}
