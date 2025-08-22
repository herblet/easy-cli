use std::io::Write;
use std::path::PathBuf;
use std::{io, process::exit, str::FromStr};

use clap::error::Error;
use clap::{parser::ValuesRef, Arg, ArgMatches};
use clap_complete::{generate, Shell};
use log::debug;

use crate::model::Command;
use crate::transform::ToCliCommand;
use model::HasSubCommands;
use model::Model;

mod model;
mod utils;

mod builder;
mod transform;

const COMPLETIONS_ARG: &str = "completions";

const CLI_SRC_ARG: &str = "SOURCE PATH";
const CLI_NAME_ARG: &str = "name";
const CLI_EXECUTED_ARG: &str = "executed";

const COMMAND_ARGS: &str = "command_args";

const DEFAULT_CLI_NAME: &str = "cli";

enum Mode {
    Executed,
    Evaluated,
    Completions(String),
}
fn main() {
    env_logger::init();

    let (cli_source, cli_args, mode) = extract_cli_source_and_args();

    let model = Model::from(&cli_source);

    let cli: clap::Command = model.to_cli();

    debug!("args-{}", cli_args.join(" "));

    match mode {
        Mode::Completions(shell) => handle_completions(cli, cli_args.iter().next().unwrap(), shell),
        Mode::Executed => execute_cli(model, cli, cli_args),
        Mode::Evaluated => write_embedded_script(model, cli, cli_args),
    }
}

fn build_embedded_script(model: Model, mut cli: clap::Command, cli_args: Vec<String>) -> Vec<u8> {
    cli.try_get_matches_from_mut(cli_args.iter()).map_or_else(
        |err| {
            // Render the error. This is also where help and usage messages are rendered, since they are represented
            // as errors in clap.
            echo_error_script(err)
        },
        |matches| {
            // render shell commands to execute the appropriate script, having setup the parameters
            exec_commands_script(model, matches)
        },
    )
}

fn write_embedded_script(model: Model, cli: clap::Command, cli_args: Vec<String>) {
    // In embedded mode, don't let clap print to stdout because stdout is to be evaled. So we need to capture
    // version and help requests (which are returned here as errors)

    let buffer = build_embedded_script(model, cli, cli_args);

    // Write the produced content to stdout
    io::stdout()
        .write_all(&buffer)
        .expect("Failed to write to stdout");
}

fn echo_error_script(err: Error) -> Vec<u8> {
    let mut buffer = Vec::new();
    write!(&mut buffer, "echo \"{}\"", err.render().ansi()).expect("Failed to write to buffer");
    buffer
}

fn exec_commands_script(model: Model, arg_matches: clap::ArgMatches) -> Vec<u8> {
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
        add_opts_and_args(current, current_command, &mut opts, &mut args);
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

    let mut buffer = Vec::new();

    writeln!(&mut buffer, "#eval").expect("Failed to write to buffer");
    writeln!(&mut buffer, "typeset -A cli_args").expect("Failed to write to buffer");
    writeln!(
        &mut buffer,
        "cli_args=({})",
        args.iter()
            .map(|arg| format!("\"{}\" \"{}\"", arg.0, arg.1))
            .collect::<Vec<String>>()
            .join(" ")
    )
    .expect("Failed to write to buffer");
    writeln!(&mut buffer, "typeset -A cli_opts").expect("Failed to write to buffer");
    writeln!(
        &mut buffer,
        "cli_opts=({})",
        opts.iter()
            .map(|opt| format!("\"{}\" {}", opt.0, opt.1))
            .collect::<Vec<String>>()
            .join(" ")
    )
    .expect("Failed to write to buffer");
    writeln!(&mut buffer, "source \"{}\"", path.to_str().unwrap())
        .expect("Failed to write to buffer");

    if current_command.get_path() == None {
        writeln!(&mut buffer, "{}", current_command.name()).expect("Failed to write to buffer");
    }

    buffer
}

fn add_opts_and_args<'a>(
    matches: &'a ArgMatches,
    command: &'a Box<dyn Command>,
    opts: &mut Vec<(&'a str, bool)>,
    args: &mut Vec<(&'a str, String)>,
) {
    matches.ids().for_each(|id| {
        let name = id.as_str();

        if let Some(option) = command.get_option(name) {
            if option.has_param {
                todo!("Handle options with args")
            } else {
                let opt_set = matches.get_flag(name);

                opts.push((name, opt_set));
            }
        }

        if let Some(_) = command.get_arg(name) {
            let value_str = matches
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
        add_opts_and_args(current, current_command, &mut opts, &mut args);

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

    let executed: bool = launcher_matches
        .get_one::<bool>(CLI_EXECUTED_ARG)
        .map(|x| *x)
        .unwrap_or(false);

    let shell_for_completions: Option<String> = launcher_matches
        .get_one::<String>(COMPLETIONS_ARG)
        .map(String::clone);

    let mode = match shell_for_completions {
        None => {
            if executed {
                Mode::Executed
            } else {
                Mode::Evaluated
            }
        }
        Some(shell) => Mode::Completions(shell),
    };

    let command_args = launcher_matches
        .get_many::<String>(COMMAND_ARGS)
        .map(|args| args.clone());

    (cli_source, build_cli_args(name, command_args), mode)
}

/// Builds the artificial command line args for use with the cli-parser for the configured cli.
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
            Arg::new(CLI_EXECUTED_ARG)
                .long(CLI_EXECUTED_ARG)
                .short('e')
                .num_args(0)
                .help("Indicates that easy cli should execute the script and pass subcommand and args to it."),
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
mod tests {
    use std::vec;

    use crate::model::{ArgType, CommandArg, EmbeddedCommand, ScriptCommand};

    use super::*;

    #[test]
    fn test_build_cli_args() {
        let bar: EmbeddedCommand = EmbeddedCommand::new(
            "bar".to_owned(),
            Option::<String>::None,
            vec![],
            vec![CommandArg::new(
                "arg1".to_owned(),
                false,
                false,
                ArgType::Unknown,
                Option::<String>::None,
            )],
        );

        let foo = ScriptCommand::new(
            "foo".to_owned(),
            None,
            PathBuf::from("/tmp/foo.sh"),
            vec![],
            vec![],
            vec![Box::new(bar)],
        );

        let model = Model::new(vec![Box::new(foo)]);
        let command = model.to_cli();

        // capture the ouput produced by embedded_commands
        let out = build_embedded_script(
            model,
            command,
            vec![
                "blah".to_owned(),
                "foo".to_owned(),
                "bar".to_owned(),
                "arg1Val".to_owned(),
            ],
        );

        let out_str = String::from_utf8(out).expect("Failed to convert to string");
        assert_eq!(out_str, "#eval\ntypeset -A cli_args\ncli_args=(\"arg1\" \"arg1Val\")\ntypeset -A cli_opts\ncli_opts=()\nsource \"/tmp/foo.sh\"\nbar\n");
    }
}
