use std::{fs::{read_dir, File}, path::PathBuf, process::{self, exit}, io::BufReader};

use lazy_static::lazy_static;
use regex::Regex;

use crate::parse::{doc_entries, DocEntry};

pub struct Model {
    pub commands: Vec<Box<dyn Command>>,
}

/// The model of a single CLI tool.
impl Model {
    // Creates a new CLI tool based on the scripts et al. in the given directory.
    pub fn new(script_dir: &str) -> Model {
        Model {
            commands: read_dir(script_dir)
                .unwrap()
                .map(|entry| {
                    let path = entry.unwrap().path();
                    Box::new(ScriptCommand::new(path).unwrap()) as Box<dyn Command>
                })
                .collect(),
        }
    }
}

pub struct CommandDescription {
    pub name: String,
    pub description: Option<String>,
    pub args: Vec<Box<Argument>>,
    pub any_arg: bool,
}

pub trait Command {
    fn description(&self) -> &CommandDescription;

    fn exec(&self, args: Option<Vec<String>>);
}

/// A single CLI command.
pub struct ScriptCommand {
    description: CommandDescription,
    path: PathBuf,
   
}

impl ScriptCommand {
    fn new(path: PathBuf) -> Result<ScriptCommand, PathBuf> {
        lazy_static! {
            static ref FILE_SUFFIX: Regex = Regex::new(".[^.]*$").unwrap();
        }

        let path_for_result = path.clone();

        let docs = doc_entries(BufReader::new(File::open(path.clone()).unwrap()));
        
        let mut description = None;
        let mut args: Vec<Box<Argument>> = Vec::new();

        let mut any_arg = false;
        docs.into_iter().for_each({
           |entry| match entry {
               DocEntry::Description(desc) => description = Some(desc),
               DocEntry::Option(name, has_arg , description) => args.push(Box::new(Argument::new(
                   name,
                   description,
                   has_arg,
                ))),
                DocEntry::AnyArg => any_arg = true,
               _ => ()
           }
        }); 

        path_for_result
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .map(|file_name| strip_file_suffix(&file_name))
            .map(|name| CommandDescription { name, description, args, any_arg })
            .map(|description| ScriptCommand { description, path } )
            .ok_or(path_for_result)
    }
}

impl Command for ScriptCommand {
   
    fn description(&self) -> &CommandDescription {
        &self.description
    }

    fn exec(&self, args: Option<Vec<String>>) {
        let mut command = process::Command::new("sh");
        
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

}

/// Strips the file type suffix -  that is, everything after the last '.' - from the given file name.
fn strip_file_suffix(name: &str) -> String {
    lazy_static! {
        static ref FILE_SUFFIX: Regex = Regex::new(".[^.]*$").unwrap();
    }

    FILE_SUFFIX.replace(&name, "").to_string()
}

pub struct Argument {
    pub name: String,
    pub description: String,
    pub has_args: bool,
}

impl Argument {
    pub fn new(name: String, description: String, has_args: bool) -> Argument {
        Argument { name, description, has_args }
    }
}

#[cfg(test)]
mod test {
    use std::fs::File;

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
}
