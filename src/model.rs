use std::{
    path::PathBuf,
    process::{self, exit},
};
use std::fs::read_dir;
use std::path::Path;

use lazy_static::lazy_static;
use regex::Regex;

use crate::builder::build_script_command;

lazy_static! {
    pub static ref SUB_COMMAND: Regex =
        Regex::new(r"# @sub: *(?P<sub>\w+) *(?P<path>\S.+)?").expect("Failed to compile regex");
    pub static ref IGNORE: Regex =
        Regex::new(r"# @ignore-at-root").expect("Failed to compile regex");
}

pub struct Model {
    pub commands: Vec<Box<dyn Command>>,
}

pub trait HasSubCommands {
    fn get_command(&self, name: &str) -> Option<&Box<dyn Command>>;
}

/// The model of a single CLI tool.
impl Model {
    pub fn new(commands: Vec<Box<dyn Command>>) -> Model {
        Model { commands }
    }
}

impl<P: AsRef<Path>> From<P> for Model {
    fn from(path: P) -> Self {
        let commands = read_dir(path)
            .map(|scripts| {
                scripts
                    .filter_map(|entry| {
                        entry
                            .ok()
                            .filter(|entry| {
                                entry
                                    .file_type()
                                    .ok()
                                    .map_or(false, |file_type| file_type.is_file())
                            })
                            .map(|entry| {
                                let path = entry.path();
                                build_script_command(path)
                                    .ok()
                                    .flatten()
                                    .map(|command| Box::new(command) as Box<dyn Command>)
                            })
                            .flatten()
                    })
                    .collect()
            })
            .unwrap_or(Vec::new());
        Model::new(commands)
    }
}

impl HasSubCommands for Model {
    fn get_command(&self, name: &str) -> Option<&Box<dyn Command>> {
        self.commands.iter().find(|command| command.name() == name)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArgType {
    Unknown,
    Path,
    File,
    Dir,
}

impl From<&str> for ArgType {
    fn from(s: &str) -> ArgType {
        if s.eq_ignore_ascii_case("path") {
            ArgType::Path
        } else if s.eq_ignore_ascii_case("file") {
            ArgType::File
        } else if s.eq_ignore_ascii_case("dir") {
            ArgType::Dir
        } else {
            ArgType::Unknown
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandArg {
    pub name: String,
    pub optional: bool,
    pub var_arg: bool,
    pub arg_type: ArgType,
    pub description: Option<String>,
}

impl CommandArg {
    pub fn new<S, T>(
        name: S,
        optional: bool,
        var_arg: bool,
        arg_type: ArgType,
        description: Option<T>,
    ) -> Self
    where
        S: Into<String>,
        T: Into<String>,
    {
        CommandArg {
            name: name.into(),
            optional,
            var_arg,
            arg_type,
            description: description.map(Into::into),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandOption {
    pub name: String,
    pub short: Option<char>,
    pub has_param: bool,
    pub description: Option<String>,
}

impl CommandOption {
    pub fn new<S, T>(name: S, short: Option<char>, has_param: bool, description: Option<T>) -> Self
    where
        S: Into<String>,
        T: Into<String>,
    {
        CommandOption {
            name: name.into(),
            short,
            has_param,
            description: description.map(Into::into),
        }
    }
}

pub(crate) trait Command {
    fn name(&self) -> &str;

    fn description(&self) -> Option<&str> {
        None
    }

    fn exec(&self, args: Option<Vec<String>>);

    fn sub_commands(&self) -> &Vec<Box<dyn Command>>;

    fn options(&self) -> &Vec<CommandOption>;

    fn args(&self) -> &Vec<CommandArg>;

    fn get_option(&self, name: &str) -> Option<&CommandOption> {
        self.options().iter().find(|option| option.name == name)
    }

    fn get_arg(&self, name: &str) -> Option<&CommandArg> {
        self.args().iter().find(|arg| arg.name == name)
    }
    fn get_path(&self) -> Option<&PathBuf>;
}

/// A single CLI command.
pub struct ScriptCommand {
    pub name: String,
    pub description: Option<String>,
    sub_commands: Vec<Box<dyn Command>>,
    path: PathBuf,
    options: Vec<CommandOption>,
    args: Vec<CommandArg>,
}

impl ScriptCommand {
    pub fn new(
        name: String,
        description: Option<String>,
        path: PathBuf,
        options: Vec<CommandOption>,
        args: Vec<CommandArg>,
        sub_commands: Vec<Box<dyn Command>>,
    ) -> ScriptCommand {
        ScriptCommand {
            name,
            description,
            path,
            options,
            args,
            sub_commands,
        }
    }
}

impl<T> HasSubCommands for T
where
    T: AsRef<dyn Command>,
{
    fn get_command(&self, name: &str) -> Option<&Box<dyn Command>> {
        let command: &dyn Command = self.as_ref();
        command
            .sub_commands()
            .iter()
            .find(|command| command.name() == name)
    }
}

impl Command for ScriptCommand {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    fn exec(&self, args: Option<Vec<String>>) {
        let mut command = process::Command::new(self.path.to_str().unwrap());

        args.iter().flat_map(|args| args.iter()).for_each(|arg| {
            command.arg(arg);
        });

        let output = command.spawn();

        match output {
            Ok(mut child) => {
                child.wait().unwrap();
                exit(0);
            }
            Err(e) => {
                eprintln!("{}", "Error in executing command : ");
                eprintln!("{}", e);
                exit(1);
            }
        }
    }

    fn sub_commands(&self) -> &Vec<Box<dyn Command>> {
        &self.sub_commands
    }

    fn options(&self) -> &Vec<CommandOption> {
        &self.options
    }

    fn args(&self) -> &Vec<CommandArg> {
        &self.args
    }
    fn get_path(&self) -> Option<&PathBuf> {
        Some(&self.path)
    }
}

pub struct EmbeddedCommand {
    name: String,
    description: Option<String>,
    options: Vec<CommandOption>,
    args: Vec<CommandArg>,
    sub_commands: Vec<Box<dyn Command>>,
}

impl EmbeddedCommand {
    pub fn new<S, T>(
        name: S,
        description: Option<T>,
        options: Vec<CommandOption>,
        args: Vec<CommandArg>,
    ) -> EmbeddedCommand
    where
        S: Into<String>,
        T: Into<String>,
    {
        EmbeddedCommand {
            name: name.into(),
            description: description.map(Into::into),
            options,
            args,
            sub_commands: vec![],
        }
    }
}

impl Command for EmbeddedCommand {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    fn exec(&self, _args: Option<Vec<String>>) {
        // The handling of sub-command execution is currently handled by the script
        unimplemented!()
    }

    fn sub_commands(&self) -> &Vec<Box<dyn Command>> {
        self.sub_commands.as_ref()
    }

    fn options(&self) -> &Vec<CommandOption> {
        &self.options
    }

    fn args(&self) -> &Vec<CommandArg> {
        &self.args
    }

    fn get_path(&self) -> Option<&PathBuf> {
        None
    }
}

#[cfg(test)]
pub(crate) mod test {
    use std::fs::File;
    use std::io::Write;

    pub const NO_DESCRIPTION: Option<String> = None;

    #[test]
    fn build_model_lists_scripts() {
        let test_dir = tempfile::tempdir().unwrap();

        let script1_path = test_dir.path().join("script1.sh");

        File::create(&script1_path)
            .expect(format!("Unable to create file {}", script1_path.to_str().unwrap()).as_str());

        let script2_path = test_dir.path().join("script2.sh");
        File::create(&script2_path)
            .expect(format!("Unable to create file {}", script2_path.to_str().unwrap()).as_str());

        let model = super::Model::from(test_dir.path());

        assert_eq!(model.commands.len(), 2);

        let mut names: Vec<String> = model
            .commands
            .into_iter()
            .map(|command| command.name().to_owned())
            .collect();

        names.sort();

        assert_eq!(names.join(","), "script1,script2");
    }

    #[test]
    fn build_model_filters_directories_scripts() {
        let test_dir = tempfile::tempdir().unwrap();

        let script1_path = test_dir.path().join("script1.sh");

        File::create(&script1_path)
            .expect(format!("Unable to create file {}", script1_path.to_str().unwrap()).as_str());

        let subdir_path = test_dir.path().join("subdir");
        // Create a directory 'subdir'
        std::fs::create_dir(&subdir_path)
            .expect(format!("Unable to create directory {}", subdir_path.to_str().unwrap()).as_str());

        let model = super::Model::from(test_dir.path());

        assert_eq!(model.commands.len(), 1);
        assert_eq!(model.commands[0].name(), "script1");
    }


    #[test]
    fn build_model_includes_function_commands() {
        let test_dir = tempfile::tempdir().unwrap();

        let script1_path = test_dir.path().join("script1.sh");

        File::create(&script1_path)
            .unwrap()
            .write("# @sub sub1\nfunction sub1(){}\n# @sub sub2\nfunction sub2(){}\n".as_bytes())
            .expect(format!("Unable to create file {}", script1_path.to_str().unwrap()).as_str());

        let model = super::Model::from(test_dir.path());

        assert_eq!(model.commands.len(), 1);
        assert_eq!(model.commands[0].sub_commands().len(), 2);

        let mut names: Vec<String> = model.commands[0]
            .sub_commands()
            .into_iter()
            .map(|command| command.name().to_owned())
            .collect();

        names.sort();

        assert_eq!(names.join(","), "sub1,sub2");
    }

    #[test]
    fn build_model_includes_script_commands() {
        let test_dir = tempfile::tempdir().unwrap();

        let script1_path = test_dir.path().join("script1.sh");

        File::create(&script1_path)
            .unwrap()
            .write(
                "# @sub sub1\nfunction sub1(){}\n# @sub sub2 script2.sh\nfunction sub2(){}\n"
                    .as_bytes(),
            )
            .expect(format!("Unable to create file {}", script1_path.to_str().unwrap()).as_str());

        let script2_path = test_dir.path().join("script2.sh");
        File::create(&script2_path)
            .expect(format!("Unable to create file {}", script2_path.to_str().unwrap()).as_str())
            .write("# @ignore-at-root\n".as_bytes())
            .expect(format!("Unable to write file {}", script2_path.to_str().unwrap()).as_str());

        let model = super::Model::from(test_dir.path());

        assert_eq!(model.commands.len(), 1);
        assert_eq!(model.commands[0].sub_commands().len(), 2);

        let mut names: Vec<String> = model.commands[0]
            .sub_commands()
            .into_iter()
            .map(|command| command.name().to_owned())
            .collect();

        names.sort();

        assert_eq!(names.join(","), "sub1,sub2");
    }

    #[test]
    fn arg_type_from_str() {
        assert_eq!(super::ArgType::from("path"), super::ArgType::Path);
        assert_eq!(super::ArgType::from("file"), super::ArgType::File);
        assert_eq!(super::ArgType::from("dir"), super::ArgType::Dir);

        // It is case-insensitive
        assert_eq!(super::ArgType::from("Path"), super::ArgType::Path);

        // Any other value is unknown
        assert_eq!(super::ArgType::from("foo"), super::ArgType::Unknown);
        assert_eq!(super::ArgType::from("bar"), super::ArgType::Unknown);
    }
}
