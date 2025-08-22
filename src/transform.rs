//! Traits and implementations for transforming the internal model into a clap command
use clap::builder::StringValueParser;
use clap::{Arg, ArgAction, ValueHint};

use crate::model::Command;
use crate::model::{ArgType, CommandArg, CommandOption, Model};

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

        let make_opts_global = self.has_sub_commands();

        // Add the Options first
        cli_command = self
            .options()
            .iter()
            .map(|option| option.to_arg(make_opts_global))
            .chain(
                // Then add the Arguments; never global (in fact, a command with subcommands should not have args)
                self.args().iter().map(|arg| arg.to_arg(false)),
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
    fn to_arg(&self, global: bool) -> Arg;
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
    fn to_arg(&self, _: bool) -> Arg {
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
    fn to_arg(&self, global: bool) -> Arg {
        let mut cli_option = Arg::new(self.name.to_owned())
            .global(global)
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
    use crate::model::test::NO_DESCRIPTION;
    use crate::model::{ArgType, EmbeddedCommand, ScriptCommand};

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

        assert_eq!(arg.to_arg(false).get_id().as_str(), "TestArg");
    }

    #[test]
    fn non_opt_arg_is_required() {
        let arg = CommandArg::new("TestArg", false, false, ArgType::Unknown, NO_DESCRIPTION);

        assert!(arg.to_arg(false).is_required_set());
    }

    #[test]
    fn opt_arg_not_required() {
        let arg = CommandArg::new("TestArg", true, false, ArgType::Unknown, NO_DESCRIPTION);

        assert!(!arg.to_arg(false).is_required_set());
    }

    #[test]
    fn non_var_arg_transfers() {
        let arg = CommandArg::new("TestArg", false, false, ArgType::Unknown, NO_DESCRIPTION);

        assert!(!arg.to_arg(false).is_trailing_var_arg_set());
    }

    #[test]
    fn var_arg_transfers() {
        let arg = CommandArg::new("TestArg", false, true, ArgType::Unknown, NO_DESCRIPTION);

        assert!(arg.to_arg(false).is_trailing_var_arg_set());
    }

    #[test]
    fn var_arg_accepts_values() {
        let arg = CommandArg::new("TestArg", false, true, ArgType::Unknown, NO_DESCRIPTION);

        assert_eq!(arg.to_arg(false).get_num_args().unwrap(), (0..).into());
    }

    #[test]
    fn no_desription_no_help() {
        let arg = CommandArg::new("TestArg", false, true, ArgType::Unknown, NO_DESCRIPTION);

        assert!(arg.to_arg(false).get_help().is_none());
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
            arg.to_arg(false).get_help().unwrap().to_string().as_str(),
            "My description"
        );
    }

    #[test]
    fn type_is_value_hint() {
        let arg = CommandArg::new("TestArg", false, true, ArgType::Dir, Some("My description"));

        assert_eq!(arg.to_arg(false).get_value_hint(), ValueHint::DirPath);
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

    fn script_command(
        opts: Vec<CommandOption>,
        args: Vec<CommandArg>,
        sub: Vec<Box<dyn Command>>,
    ) -> ScriptCommand {
        ScriptCommand::new(
            "test".to_string(),
            Some("echo test".to_string()),
            "Test command".into(),
            opts,
            args,
            sub,
        )
    }

    fn embedded_command(
        idx: u32,
        opts: Vec<CommandOption>,
        args: Vec<CommandArg>,
    ) -> EmbeddedCommand {
        EmbeddedCommand::new(
            format!("sub{}", idx),
            Some(format!("embedded sub{}", idx)),
            opts,
            args,
        )
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
        let command = script_command(vec![], vec![arg("arg1"), arg("arg2")], vec![]);

        let cli_command: CliCommand = command.to_cli();

        let args = cli_command.get_arguments().collect::<Vec<_>>();

        assert_eq!(args.len(), 2);
        assert_eq!(args[0].get_id().as_str(), "arg1");
        assert_eq!(args[1].get_id().as_str(), "arg2");
    }

    #[test]
    fn to_cli_adds_opts() {
        let command = script_command(vec![opt("foo"), opt("bar")], vec![], vec![]);

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
        let command = script_command(
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
        let command = script_command(
            vec![],
            vec![],
            vec![
                Box::new(embedded_command(1, vec![], vec![])),
                Box::new(embedded_command(2, vec![], vec![])),
            ],
        );

        let cli_command: CliCommand = command.to_cli();

        let subs = cli_command.get_subcommands().collect::<Vec<_>>();

        assert_eq!(subs.len(), 2);
        assert_eq!(subs[0].get_name(), "sub1");
        assert_eq!(subs[1].get_name(), "sub2");
    }

    #[test]
    fn to_cli_makes_super_options_global() {
        let command = ScriptCommand::new(
            "test".to_string(),
            Some("echo test".to_string()),
            "Test command".into(),
            vec![opt("foo"), opt("bar")],
            vec![],
            vec![Box::new(embedded_command(1, vec![], vec![]))],
        );

        let cli_command: CliCommand = command.to_cli();

        let args = cli_command.get_arguments().collect::<Vec<_>>();

        assert_eq!(args.len(), 2);
        assert!(args[0].is_global_set());
        assert!(args[1].is_global_set());
    }

    #[test]
    fn to_cli_keeps_leaf_options_local() {
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
        assert!(!args[0].is_global_set());
        assert!(!args[1].is_global_set());
    }
}
