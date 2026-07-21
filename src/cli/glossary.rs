mod due;
mod grade;
mod mark;
mod search;
mod term;

use clap::{Args, Subcommand};

use super::output::CliCommandError;
use super::runtime::run_headless_command;
use due::GlossaryDueArgs;
use grade::GlossaryGradeArgs;
use mark::GlossaryMarkArgs;
use search::GlossarySearchArgs;
use term::{GlossaryListArgs, GlossaryShowArgs};

#[derive(Debug, Clone, Args)]
pub(crate) struct GlossaryArgs {
    #[command(subcommand)]
    pub(crate) command: GlossaryCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub(crate) enum GlossaryCommand {
    /// List indexed glossary terms.
    List(GlossaryListArgs),
    /// Search glossary terms by title, synonym, ref, or definition text.
    Search(GlossarySearchArgs),
    /// Show one exact glossary term.
    Show(GlossaryShowArgs),
    /// List glossary terms due for review.
    Due(GlossaryDueArgs),
    /// Grade a term and reschedule it with SM-2.
    Grade(GlossaryGradeArgs),
    /// Mark an existing note as a glossary term.
    Mark(GlossaryMarkArgs),
}

pub(crate) fn run_glossary(args: &GlossaryArgs) -> Result<(), CliCommandError> {
    match &args.command {
        GlossaryCommand::List(command) => run_headless_command(command),
        GlossaryCommand::Search(command) => run_headless_command(command),
        GlossaryCommand::Show(command) => run_headless_command(command),
        GlossaryCommand::Due(command) => run_headless_command(command),
        GlossaryCommand::Grade(command) => run_headless_command(command),
        GlossaryCommand::Mark(command) => run_headless_command(command),
    }
}
