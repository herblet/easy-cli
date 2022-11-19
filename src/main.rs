use std::process::exit;

use clap::{Arg, parser::ValuesRef};
use model::Model;

mod model;

const CLI_SRC_ARG: &str = "SOURCE PATH";
const CLI_NAME_ARG: &str = "name";
const COMMAND_ARGS: &str = "command_args";

const DEFAULT_CLI_NAME: &str = "cli";

fn main() {
    let (cli_source, cli_args) = extract_cli_source_and_args();

    let model = Model::new(&cli_source);

    let cli = to_cli(&model);

    let arg_matches = cli.get_matches_from(cli_args.iter());

    let (script_to_call, matches) = arg_matches.subcommand().unwrap();

    let command = model
        .commands
        .iter()
        .find(|command| command.name() == script_to_call)
        .unwrap();

    let command_args: Option<Vec<String>> = matches.get_raw("args").map(|values| {
        values.map(|value| {
            value.to_str().unwrap().to_owned()
        }).collect()
    });

    command.exec(command_args);
}

fn extract_cli_source_and_args() -> (String, Vec<String>) {

    // Create an argument-parser for easy-cli itself.
    let mut launcher_cli = launcher_cli();
    
    // Match the arguments against the launcher-cli.
    let launcher_matches = launcher_cli.get_matches_mut();

    // If a subcommand was specified it must be help, since there are no others
    if launcher_matches.subcommand_name().is_some() {
        launcher_cli.print_help().unwrap();
        exit(0);
    }

    // Determine the source path for the cli.
    let cli_source: String = launcher_matches
        .get_one::<String>(CLI_SRC_ARG)
        .unwrap(/* Since required should be fine */)
        .clone();


    // Determine the name of the cli, used in help messages.
    let name: String = launcher_matches
        .get_one::<String>(CLI_NAME_ARG)
        .map(String::clone)
        .unwrap_or(DEFAULT_CLI_NAME.to_owned());

    let command_args = launcher_matches.get_many::<String>(COMMAND_ARGS).map(|args| args.clone());

    (cli_source, build_cli_args(name, command_args))
}

/// Builds the artificial command line ares for use with the cli-parser for the configured cli.
fn build_cli_args(name: String, command_args: Option<ValuesRef<String>>) -> Vec<String> {
    // The full list of args for the cli contains the cli name...
    Box::new([name].into_iter())
    .chain(
        //... followed by all the trailing args to easy-cli.
        Box::new(command_args.into_iter().flat_map(|values|values).map(String::clone))
    ).collect()
}

/// Creates an argument-parser for easy-cli itself.
fn launcher_cli() -> clap::Command {
    clap::Command::new("easy-cli")
        .about("A launcher for scripts")
        .arg(
            Arg::new(CLI_NAME_ARG)
                .short('n')
                .long("name")
                .help("The name of the cli tool."),
        )
        .arg(
            Arg::new(CLI_SRC_ARG)
                .help("The directory containing the scripts to be called")
                .required(true),
        )
        .arg(
            Arg::new(COMMAND_ARGS)
                .allow_hyphen_values(true)
                .num_args(0..=10)
                .trailing_var_arg(true),
        )
}

fn initial_cli() -> clap::Command {
    clap::Command::new("easy-cli").version("0.1.0")
}

fn to_cli(model: &Model) -> clap::Command {
    model.commands.iter().fold(initial_cli(), |cli, command| {
        cli.subcommand(
            clap::Command::new(command.name().to_owned()).arg(
                Arg::new("args")
                    .allow_hyphen_values(true)
                    .num_args(0..=10)
                    .trailing_var_arg(true),
            ),
        )
    })
}
