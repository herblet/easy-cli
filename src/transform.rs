//! Traits and implementations for transforming the internal model into a clap command
use clap::{Arg, ArgAction, ValueHint};
use clap::builder::StringValueParser;

use crate::model::{ArgType, CommandArg, CommandOption, Model};
use crate::model::Command;

/// Convenience type alias to avoid confusion with internal Command
type CliCommand = clap::Command;

/// Trait to convert implementors to a clap Command
pub trait ToCliCommand {
    fn to_cli(&self) -> CliCommand;
}

/// Converts an entire Model to a CliCommand
impl ToCliCommand for Model {
    fn to_cli(&self) -> CliCommand {
        self.commands.iter().fold(top_level(), |cli, command| {
            cli.subcommand(command.as_ref().to_cli())
        })
    }
}

fn top_level() -> CliCommand {
    clap::Command::new("easy-cli")
        .version("0.1.0")
        .subcommand_required(true)
}

impl<C: ?Sized + Command> ToCliCommand for C {
    fn to_cli(&self) -> CliCommand {
        let mut cli_command = CliCommand::new(self.name().to_owned()).about(
            self.description()
                .map(|str| str.to_owned())
                .unwrap_or(format!("Runs the {} script", self.name())),
        );

        // Add the Options first
        cli_command = self
            .options()
            .iter()
            .map(ToArg::to_arg)
            .chain(
                // Then add the Arguments
                self.args().iter().map(ToArg::to_arg),
            )
            .fold(cli_command, CliCommand::arg);

        // Add the sub_commands
        self.sub_commands()
            .iter()
            .map(|sub| sub.as_ref().to_cli())
            .fold(cli_command, |parent, sub_command| {
                parent.subcommand(sub_command)
            })
    }
}

/// Converts an implementor to a clap Arg
trait ToArg {
    fn to_arg(&self) -> Arg;
}

trait ToValueHint {
    fn to_value_hint(&self) -> ValueHint;
}

impl ToValueHint for ArgType {
    fn to_value_hint(&self) -> ValueHint {
        match self {
            ArgType::File => ValueHint::FilePath,
            ArgType::Dir => ValueHint::DirPath,
            ArgType::Path => ValueHint::AnyPath,
            ArgType::Unknown => ValueHint::Unknown,
        }
    }
}

impl ToArg for CommandArg {
    fn to_arg(&self) -> Arg {
        let mut cli_arg = Arg::new(self.name.to_owned())
            .value_parser(StringValueParser::default())
            .required(!self.optional);

        if let Some(text) = self.description.as_ref() {
            cli_arg = cli_arg.help(text);
        }

        if self.var_arg {
            cli_arg = cli_arg.trailing_var_arg(true).num_args(0..)
        } else {
            cli_arg = cli_arg.num_args(1);
        }

        cli_arg = cli_arg.value_hint(self.arg_type.to_value_hint());

        cli_arg
    }
}

impl ToArg for CommandOption {
    fn to_arg(&self) -> Arg {
        let mut cli_option = Arg::new(self.name.to_owned())
            .short(self.short)
            .long(self.name.to_owned())
            .help(self.description.as_deref().unwrap_or("").to_string());

        if !self.has_param {
            cli_option = cli_option.num_args(0).action(ArgAction::SetTrue);
        } else {
            cli_option = cli_option.value_parser(StringValueParser::default());
        }

        cli_option
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{ArgType, EmbeddedCommand, ScriptCommand};
    use crate::model::test::NO_DESCRIPTION;

    use super::*;

    #[test]
    fn to_value_hint_maps_to_clap() {
        assert_eq!(ArgType::File.to_value_hint(), ValueHint::FilePath);
        assert_eq!(ArgType::Dir.to_value_hint(), ValueHint::DirPath);
        assert_eq!(ArgType::Path.to_value_hint(), ValueHint::AnyPath);
        assert_eq!(ArgType::Unknown.to_value_hint(), ValueHint::Unknown);
    }

    #[test]
    fn arg_transfers_name() {
        let arg = CommandArg::new("TestArg", false, false, ArgType::Unknown, NO_DESCRIPTION);

        assert_eq!(arg.to_arg().get_id().as_str(), "TestArg");
    }

    #[test]
    fn non_opt_arg_is_required() {
        let arg = CommandArg::new("TestArg", false, false, ArgType::Unknown, NO_DESCRIPTION);

        assert!(arg.to_arg().is_required_set());
    }

    #[test]
    fn opt_arg_not_required() {
        let arg = CommandArg::new("TestArg", true, false, ArgType::Unknown, NO_DESCRIPTION);

        assert!(!arg.to_arg().is_required_set());
    }

    #[test]
    fn non_var_arg_transfers() {
        let arg = CommandArg::new("TestArg", false, false, ArgType::Unknown, NO_DESCRIPTION);

        assert!(!arg.to_arg().is_trailing_var_arg_set());
    }

    #[test]
    fn var_arg_transfers() {
        let arg = CommandArg::new("TestArg", false, true, ArgType::Unknown, NO_DESCRIPTION);

        assert!(arg.to_arg().is_trailing_var_arg_set());
    }

    #[test]
    fn var_arg_accepts_values() {
        let arg = CommandArg::new("TestArg", false, true, ArgType::Unknown, NO_DESCRIPTION);

        assert_eq!(arg.to_arg().get_num_args().unwrap(), (0..).into());
    }

    #[test]
    fn no_desription_no_help() {
        let arg = CommandArg::new("TestArg", false, true, ArgType::Unknown, NO_DESCRIPTION);

        assert!(arg.to_arg().get_help().is_none());
    }

    #[test]
    fn description_maps_to_help() {
        let arg = CommandArg::new(
            "TestArg",
            false,
            true,
            ArgType::Unknown,
            Some("My description"),
        );

        assert_eq!(
            arg.to_arg().get_help().unwrap().to_string().as_str(),
            "My description"
        );
    }

    #[test]
    fn type_is_value_hint() {
        let arg = CommandArg::new("TestArg", false, true, ArgType::Dir, Some("My description"));

        assert_eq!(arg.to_arg().get_value_hint(), ValueHint::DirPath);
    }

    #[test]
    fn from_creates_easy_cli_command() {
        let model = Model::new(vec![]);

        let command: CliCommand = model.to_cli();

        assert_eq!(command.get_name(), "easy-cli");
        assert_eq!(command.get_version(), Some("0.1.0"));
    }

    #[test]
    fn from_creates_easy_cli_command_with_subcommands() {
        let command = ScriptCommand::new(
            "test".to_string(),
            Some("echo test".to_string()),
            "Test command".into(),
            vec![],
            vec![],
            vec![],
        );
        let model = Model::new(vec![Box::new(command)]);

        let cli_command: CliCommand = model.to_cli();

        assert_eq!(1, cli_command.get_subcommands().count());
    }

    fn arg(name: &str) -> CommandArg {
        CommandArg::new(name, false, false, ArgType::Unknown, NO_DESCRIPTION)
    }

    fn opt(name: &str) -> CommandOption {
        CommandOption::new(
            name,
            Some(name.chars().next().unwrap()),
            false,
            NO_DESCRIPTION,
        )
    }
    #[test]
    fn to_cli_adds_args() {
        let command = ScriptCommand::new(
            "test".to_string(),
            Some("echo test".to_string()),
            "Test command".into(),
            vec![],
            vec![arg("arg1"), arg("arg2")],
            vec![],
        );

        let cli_command: CliCommand = command.to_cli();

        let args = cli_command.get_arguments().collect::<Vec<_>>();

        assert_eq!(args.len(), 2);
        assert_eq!(args[0].get_id().as_str(), "arg1");
        assert_eq!(args[1].get_id().as_str(), "arg2");
    }

    #[test]
    fn to_cli_adds_opts() {
        let command = ScriptCommand::new(
            "test".to_string(),
            Some("echo test".to_string()),
            "Test command".into(),
            vec![opt("foo"), opt("bar")],
            vec![],
            vec![],
        );

        let cli_command: CliCommand = command.to_cli();

        let args = cli_command.get_arguments().collect::<Vec<_>>();

        assert_eq!(args.len(), 2);
        assert_eq!(args[0].get_id().as_str(), "foo");
        assert_eq!(args[0].get_short().unwrap(), 'f');

        assert_eq!(args[1].get_id().as_str(), "bar");
        assert_eq!(args[1].get_short().unwrap(), 'b');
    }

    #[test]
    fn to_cli_adds_opts_and_args() {
        let command = ScriptCommand::new(
            "test".to_string(),
            Some("echo test".to_string()),
            "Test command".into(),
            vec![opt("foo"), opt("bar")],
            vec![arg("arg1"), arg("arg2")],
            vec![],
        );

        let cli_command: CliCommand = command.to_cli();

        let args = cli_command.get_arguments().collect::<Vec<_>>();

        assert_eq!(args.len(), 4);

        assert_eq!(args[0].get_id().as_str(), "foo");
        assert_eq!(args[0].get_short().unwrap(), 'f');

        assert_eq!(args[1].get_id().as_str(), "bar");
        assert_eq!(args[1].get_short().unwrap(), 'b');

        assert_eq!(args[2].get_id().as_str(), "arg1");
        assert_eq!(args[3].get_id().as_str(), "arg2");
    }

    #[test]
    fn to_cli_adds_sub_commands() {
        let command = ScriptCommand::new(
            "test".to_string(),
            Some("echo test".to_string()),
            "Test command".into(),
            vec![],
            vec![],
            vec![
                Box::new(EmbeddedCommand::new(
                    "sub",
                    Some("echo sub"),
                    vec![],
                    vec![],
                )),
                Box::new(EmbeddedCommand::new(
                    "sub2",
                    Some("echo sub2"),
                    vec![],
                    vec![],
                )),
            ],
        );

        let cli_command: CliCommand = command.to_cli();

        let subs = cli_command.get_subcommands().collect::<Vec<_>>();

        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0].get_name(), "sub");
        assert_eq!(subs[1].get_name(), "sub2");
    }
}
