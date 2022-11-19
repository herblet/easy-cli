use std::{env::args, process::{self, exit}};

use clap::Arg;
use model::Model;

mod model;


fn main() {

    let mut arg_iter = args();
    arg_iter.next();

    let script_dir = arg_iter.next().expect("A script directory must be specified as the first argument"); 
    let model = Model::new(&script_dir);

    let cli = to_cli(&model);

    let mut cli_iter = args();
    cli_iter.next();

    let arg_matches = cli.get_matches_from(cli_iter);

    let (script_to_call, matches) = arg_matches.subcommand().unwrap();

    let args: Option<Vec<String>> = matches.get_raw("args").map(|values| {
        values.map(|value| {
            value.to_str().unwrap().to_owned()
        }).collect()
    });

    let command = model.commands.iter().find(|command|{command.name() == script_to_call}).unwrap();
    command.exec(args);
}

fn initial_cli() -> clap::Command {
    clap::Command::new("easy-cli")
        .version("0.1.0")
}

fn to_cli(model: &Model) -> clap::Command {
    model.commands.iter().fold(initial_cli(), |cli, command| {
        cli.subcommand(clap::Command::new(command.name().to_owned()).arg(Arg::new("args").allow_hyphen_values(true).num_args(0..=10).trailing_var_arg(true)))
    })
}