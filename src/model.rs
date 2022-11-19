use std::{fs::read_dir, path::PathBuf, process::{self, exit}};

use lazy_static::lazy_static;
use regex::Regex;

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

pub trait Command {
    fn name(&self) -> &str;

    fn exec(&self, args: Option<Vec<String>>);
}

/// A single CLI command.
pub struct ScriptCommand {
    pub name: String,
    path: PathBuf,
}

impl ScriptCommand {
    fn new(path: PathBuf) -> Result<ScriptCommand, PathBuf> {
        lazy_static! {
            static ref FILE_SUFFIX: Regex = Regex::new(".[^.]*$").unwrap();
        }

        let path_for_result = path.clone();
    
        path_for_result
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .map(|file_name| strip_file_suffix(&file_name))
            .map(|name| ScriptCommand { name, path })
            .ok_or(path_for_result)
    }
}

impl Command for ScriptCommand {
    fn name(&self) -> &str {
        self.name.as_str()
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
