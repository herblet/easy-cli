use std::fs::read_dir;
use std::{
    path::PathBuf,
    process::{self, exit},
};

use crate::builder::build_script_command;
use lazy_static::lazy_static;
use regex::Regex;

lazy_static! {
    pub static ref SUB_COMMAND: Regex =
        Regex::new(r"# @sub: *(?P<sub>\w+) *(?P<path>\S.+)?").expect("Failed to compile regex");
    pub static ref IGNORE: Regex =
        Regex::new(r"# @ignore-at-root").expect("Failed to compile regex");
}

pub struct Model {
    pub commands: Vec<Box<dyn Command>>,
}

/// The model of a single CLI tool.
impl Model {
    // Creates a new CLI tool based on the scripts et al. in the given directory.
    pub fn new(script_dir: &str) -> Model {
        Model {
            commands: read_dir(script_dir)
                .map(|scripts| {
                    scripts
                        .filter_map(|entry| {
                            entry
                                .ok()
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
                .unwrap_or(Vec::new()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandArg {
    pub name: String,
    pub optional: bool,
    pub var_arg: bool,
    pub description: Option<String>,
}

impl CommandArg {
    pub fn new(name: String, optional: bool, var_arg: bool, description: Option<String>) -> Self {
        CommandArg {
            name,
            optional,
            var_arg,
            description,
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
    pub fn new(
        name: String,
        short: Option<char>,
        has_param: bool,
        description: Option<String>,
    ) -> Self {
        CommandOption {
            name,
            short,
            has_param,
            description,
        }
    }
}

pub trait Command {
    fn name(&self) -> &str;

    fn description(&self) -> Option<&str> {
        None
    }

    fn exec(&self, args: Option<Vec<String>>);

    fn sub_commands(&self) -> &Vec<Box<dyn Command>>;

    fn options(&self) -> &Vec<CommandOption>;

    fn args(&self) -> &Vec<CommandArg>;
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

impl Command for ScriptCommand {
    fn name(&self) -> &str {
        self.name.as_str()
    }

    fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    fn exec(&self, args: Option<Vec<String>>) {
        let mut command = process::Command::new("zsh");

        command.arg(self.path.clone());

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
}

pub struct EmbeddedCommand {
    name: String,
    descripton: Option<String>,
    options: Vec<CommandOption>,
    args: Vec<CommandArg>,
    sub_commands: Vec<Box<dyn Command>>,
}

impl EmbeddedCommand {
    pub fn new(
        name: String,
        descripton: Option<String>,
        options: Vec<CommandOption>,
        args: Vec<CommandArg>,
    ) -> EmbeddedCommand {
        EmbeddedCommand {
            name,
            descripton,
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
        self.descripton.as_deref()
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
}

#[cfg(test)]
mod test {
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn build_model_lists_scripts() {
        let test_dir = tempfile::tempdir().unwrap();

        let script1_path = test_dir.path().join("script1.sh");

        File::create(&script1_path)
            .expect(format!("Unable to create file {}", script1_path.to_str().unwrap()).as_str());

        let script2_path = test_dir.path().join("script2.sh");
        File::create(&script2_path)
            .expect(format!("Unable to create file {}", script2_path.to_str().unwrap()).as_str());

        let model = super::Model::new(test_dir.path().to_str().unwrap());

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
    fn build_model_includes_function_commands() {
        let test_dir = tempfile::tempdir().unwrap();

        let script1_path = test_dir.path().join("script1.sh");

        File::create(&script1_path)
            .unwrap()
            .write("# @sub sub1\nfunction sub1(){}\n# @sub sub2\nfunction sub2(){}\n".as_bytes())
            .expect(format!("Unable to create file {}", script1_path.to_str().unwrap()).as_str());

        let model = super::Model::new(test_dir.path().to_str().unwrap());

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

        let model = super::Model::new(test_dir.path().to_str().unwrap());

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
}
