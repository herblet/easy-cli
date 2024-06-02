use std::path::PathBuf;
use std::{io, process::exit, str::FromStr};

use clap::{parser::ValuesRef, Arg};
use clap_complete::{generate, Shell};
use log::debug;

use model::HasSubCommands;
use model::Model;

use crate::transform::ToCliCommand;

mod model;
mod utils;

mod builder;
mod transform;

const COMPLETIONS_ARG: &str = "completions";

const CLI_SRC_ARG: &str = "SOURCE PATH";
const CLI_NAME_ARG: &str = "name";
const CLI_EMBEDDED_ARG: &str = "embedded";

const COMMAND_ARGS: &str = "command_args";

const DEFAULT_CLI_NAME: &str = "cli";

enum Mode {
    Executed,
    Embedded,
    Completions(String),
    EmbeddedCompletions(String),
}
fn main() {
    env_logger::init();

    let (cli_source, cli_args, mode) = extract_cli_source_and_args();

    let model = Model::from(&cli_source);

    let cli: clap::Command = model.to_cli();

    debug!("args-{}", cli_args.join(" "));

    match mode {
        Mode::Completions(shell) => handle_completions(cli, cli_args.iter().next().unwrap(), shell),
        Mode::EmbeddedCompletions(shell) => {
            handle_completions(cli, cli_args.iter().next().unwrap(), shell)
        }
        Mode::Executed => execute_cli(model, cli, cli_args),
        Mode::Embedded => embedded_commands(model, cli, cli_args),
    }
}

fn embedded_commands(model: Model, cli: clap::Command, cli_args: Vec<String>) {
    let arg_matches = cli.get_matches_from(cli_args.iter());

    let (script_to_call, matches) = arg_matches.subcommand().unwrap();

    let command = model.get_command(script_to_call).unwrap();

    let mut current_command = command;
    let mut path: &PathBuf = current_command.get_path().unwrap();

    debug!("args-{}", command.name());

    let mut current = matches;

    let mut opts = Vec::<(&str, bool)>::new();
    let mut args = Vec::<(&str, String)>::new();

    // recursively collect subcommand names into a vector while it is not None
    let mut result = vec![];
    loop {
        current.ids().for_each(|id| {
            let name = id.as_str();

            if let Some(option) = current_command.get_option(name) {
                if option.has_param {
                    todo!("Handle options with args")
                } else {
                    let opt_set = current.get_flag(name);

                    opts.push((name, opt_set));
                }
            }

            if let Some(_) = current_command.get_arg(name) {
                let value_str = current
                    .get_raw(name)
                    .map(|value| {
                        let strings = value
                            .map(|v| v.to_str().unwrap().to_string())
                            .collect::<Vec<String>>();
                        strings.join(",")
                    })
                    .unwrap_or("".to_string());
                args.push((name, value_str));
            }
        });
        match current.subcommand() {
            None => break,
            Some((sub_name, sub_matches)) => {
                result.push(sub_name.to_owned());
                current = sub_matches;
                current_command = current_command.get_command(sub_name).unwrap();

                if let Some(new_path) = current_command.get_path() {
                    path = new_path
                }
            }
        }
    }

    // echo "cli_args=(\"one\" \"two\" \"three\")"
    // echo "typeset -A cli_opts"
    // echo "cli_opts=(\"one\" 1 \"two\" 2 \"three\" 3)"
    // echo "source \"/Users/toby/git_new/herblet/easy-cli/example/callee.sh\""
    // echo "callee_one"

    println!("typeset -A cli_args");
    println!(
        "cli_args=({})",
        args.iter()
            .map(|arg| format!("\"{}\" \"{}\"", arg.0, arg.1))
            .collect::<Vec<String>>()
            .join(" ")
    );
    println!("typeset -A cli_opts");
    println!(
        "cli_opts=({})",
        opts.iter()
            .map(|opt| format!("\"{}\" {}", opt.0, opt.1))
            .collect::<Vec<String>>()
            .join(" ")
    );
    println!("source \"{}\"", path.to_str().unwrap());

    if current_command.get_path() == None {
        println!("{}", current_command.name());
    }

    // Collect the args again, to pass to the script
    // current
    //     .ids()
    //     .filter_map(|id| current.get_raw(id.as_str()))
    //     .for_each(|args| {
    //         args.for_each(|arg| {
    //             result.push(arg.to_str().unwrap().to_owned());
    //         });
    //     });
    //
    // command.exec(Some(result));
}

fn execute_cli(model: Model, cli: clap::Command, cli_args: Vec<String>) {
    let arg_matches = cli.get_matches_from(cli_args.iter());

    let (script_to_call, matches) = arg_matches.subcommand().unwrap();

    let command = model.get_command(script_to_call).unwrap();

    let current_command = command;

    debug!("args-{}", command.name());

    let mut current = matches;

    let mut opts = Vec::<(&str, bool)>::new();
    let mut args = Vec::<(&str, String)>::new();

    // recursively collect subcommand names into a vector while it is not None
    let mut result = vec![];
    loop {
        current.ids().for_each(|id| {
            let name = id.as_str();

            if let Some(option) = current_command.get_option(name) {
                if option.has_param {
                    todo!("Handle options with args")
                } else {
                    let opt_set = current.get_flag(name);

                    opts.push((name, opt_set));
                }
            }

            if let Some(_) = current_command.get_arg(name) {
                let value_str = current
                    .get_raw(name)
                    .map(|value| {
                        let strings = value
                            .map(|v| v.to_str().unwrap().to_string())
                            .collect::<Vec<String>>();
                        strings.join(",")
                    })
                    .unwrap_or("".to_string());
                args.push((name, value_str));
            }
        });
        match current.subcommand() {
            None => break,
            Some((sub_name, sub_matches)) => {
                result.push(sub_name.to_owned());
                current = sub_matches;
            }
        }
    }

    // Collect the args again, to pass to the script
    current
        .ids()
        .filter_map(|id| current.get_raw(id.as_str()))
        .for_each(|args| {
            args.for_each(|arg| {
                result.push(arg.to_str().unwrap().to_owned());
            });
        });

    command.exec(Some(result));
}

fn extract_cli_source_and_args() -> (String, Vec<String>, Mode) {
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

    let embedded: bool = launcher_matches
        .get_one::<bool>(CLI_EMBEDDED_ARG)
        .map(|x| *x)
        .unwrap_or(false);

    let shell_for_completions: Option<String> = launcher_matches
        .get_one::<String>(COMPLETIONS_ARG)
        .map(String::clone);

    let mode = match shell_for_completions {
        None => {
            if embedded {
                Mode::Embedded
            } else {
                Mode::Executed
            }
        }
        Some(shell) => {
            if embedded {
                Mode::EmbeddedCompletions(shell)
            } else {
                Mode::Completions(shell)
            }
        }
    };

    let command_args = launcher_matches
        .get_many::<String>(COMMAND_ARGS)
        .map(|args| args.clone());

    (cli_source, build_cli_args(name, command_args), mode)
}

/// Builds the artificial command line ares for use with the cli-parser for the configured cli.
fn build_cli_args(name: String, command_args: Option<ValuesRef<String>>) -> Vec<String> {
    // The full list of args for the cli contains the cli name...
    Box::new([name].into_iter())
        .chain(
            //... followed by all the trailing args to easy-cli.
            Box::new(
                command_args
                    .into_iter()
                    .flat_map(|values| values)
                    .map(String::clone),
            ),
        )
        .collect()
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
            Arg::new(CLI_EMBEDDED_ARG)
                .long(CLI_EMBEDDED_ARG)
                .short('e')
                .num_args(0)
                .help("Indicates that easy cli is embedded in a script."),
        )
        .arg(
            Arg::new(CLI_SRC_ARG)
                .help("The directory containing the scripts to be called")
                .required(true),
        )
        .arg(
            Arg::new(COMPLETIONS_ARG)
                .long(COMPLETIONS_ARG)
                .help("Generate shell completions")
                .value_name("shell"),
        )
        .arg(
            Arg::new(COMMAND_ARGS)
                .allow_hyphen_values(true)
                .num_args(0..=10)
                .trailing_var_arg(true),
        )
}

fn handle_completions(mut cli: clap::Command, cli_name: &str, shell_name: String) {
    let cli_name = cli_name;

    match Shell::from_str(shell_name.as_str()) {
        Ok(shell) => {
            generate(shell, &mut cli, cli_name, &mut io::stdout());
            exit(0);
        }
        Err(e) => {
            eprintln!("Error reading shell name '{}': {}", shell_name, e);
            exit(1);
        }
    };
}

#[cfg(test)]
mod tests {}
