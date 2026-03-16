use std::fs::{read_dir, File};
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;
use std::{
    path::PathBuf,
    process::{self, exit},
};

use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::builder::build_script_command;

const CACHE_FILE: &str = ".easy-cli";

lazy_static! {
    pub static ref SUB_COMMAND: Regex =
        Regex::new(r"# @sub: *(?P<sub>\w+) *(?P<path>\S.+)?").expect("Failed to compile regex");
    pub static ref IGNORE: Regex =
        Regex::new(r"# @ignore-at-root").expect("Failed to compile regex");
}

/// Serializable representation of the Model for caching.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedModel {
    pub commands: Vec<SerializedCommand>,
}

impl SerializedModel {
    /// Make all paths relative to base_path for portability.
    fn with_relative_paths(mut self, base_path: &Path) -> Self {
        for cmd in &mut self.commands {
            cmd.make_paths_relative(base_path);
        }
        self
    }

    /// Convert to Model, resolving paths relative to base_path.
    fn into_model(self, base_path: &Path) -> Model {
        let commands: Vec<Box<dyn Command>> = self
            .commands
            .into_iter()
            .map(|c| c.into_command(base_path))
            .collect();
        Model::new(commands)
    }
}

/// Serializable representation of a command (ScriptCommand or EmbeddedCommand).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SerializedCommand {
    Script {
        name: String,
        description: Option<String>,
        path: PathBuf,
        options: Vec<CommandOption>,
        args: Vec<CommandArg>,
        sub_commands: Vec<SerializedCommand>,
    },
    Embedded {
        name: String,
        description: Option<String>,
        options: Vec<CommandOption>,
        args: Vec<CommandArg>,
        sub_commands: Vec<SerializedCommand>,
    },
}

impl SerializedCommand {
    /// Make paths relative to base_path for portability when saving.
    fn make_paths_relative(&mut self, base_path: &Path) {
        match self {
            SerializedCommand::Script {
                path, sub_commands, ..
            } => {
                if path.is_absolute() {
                    if let Ok(rel) = path.strip_prefix(base_path) {
                        *path = rel.to_path_buf();
                    }
                }
                for cmd in sub_commands {
                    cmd.make_paths_relative(base_path);
                }
            }
            SerializedCommand::Embedded { sub_commands, .. } => {
                for cmd in sub_commands {
                    cmd.make_paths_relative(base_path);
                }
            }
        }
    }

    /// Convert to Box<dyn Command>, resolving paths relative to base_path if needed.
    fn into_command(self, base_path: &Path) -> Box<dyn Command> {
        match self {
            SerializedCommand::Script {
                name,
                description,
                path,
                options,
                args,
                sub_commands,
            } => {
                let path = if path.is_relative() {
                    base_path.join(path)
                } else {
                    path
                };
                let sub_commands: Vec<Box<dyn Command>> = sub_commands
                    .into_iter()
                    .map(|c| c.into_command(base_path))
                    .collect();
                Box::new(ScriptCommand::new(
                    name,
                    description,
                    path,
                    options,
                    args,
                    sub_commands,
                ))
            }
            SerializedCommand::Embedded {
                name,
                description,
                options,
                args,
                sub_commands,
            } => {
                let sub_commands: Vec<Box<dyn Command>> = sub_commands
                    .into_iter()
                    .map(|c| c.into_command(base_path))
                    .collect();
                Box::new(EmbeddedCommand::with_sub_commands(
                    name,
                    description,
                    options,
                    args,
                    sub_commands,
                ))
            }
        }
    }
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

    pub fn from_cache(cache_path: &Path) -> Option<Model> {
        let file = File::open(cache_path).ok()?;
        let reader = BufReader::new(file);
        let serialized: SerializedModel = serde_cbor::from_reader(reader).ok()?;
        Some(serialized.into_model(cache_path.parent().unwrap()))
    }

    /// Save Model to cache file.
    pub fn save_to_cache(self: &Model, dir_path: &Path, cache_path: &Path) {
        let serialized = SerializedModel {
            commands: self.commands.iter().map(|c| c.as_serialized()).collect(),
        }
        .with_relative_paths(dir_path);

        if let Ok(file) = File::create(cache_path) {
            let mut writer = BufWriter::new(file);
            if serde_cbor::to_writer(&mut writer, &serialized).is_ok() {
                let _ = writer.flush();
            }
        }
    }
}

impl<P: AsRef<Path>> From<P> for Model {
    fn from(path: P) -> Self {
        let path = path.as_ref().to_path_buf();
        let cache_path = path.join(CACHE_FILE);

        // Try to load from cache if it exists and is newer than all other files
        if let Some(model) = try_load_from_cache(&path, &cache_path) {
            return model;
        }

        // Build from scratch
        let commands = read_dir(&path)
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
                            .filter(|entry| {
                                // Exclude the cache file itself
                                entry.path().file_name().map_or(true, |n| n != CACHE_FILE)
                            })
                            .map(|entry| {
                                let entry_path = entry.path();
                                build_script_command(entry_path)
                                    .ok()
                                    .flatten()
                                    .map(|command| Box::new(command) as Box<dyn Command>)
                            })
                            .flatten()
                    })
                    .collect()
            })
            .unwrap_or(Vec::new());
        let model = Model::new(commands);

        // Save to cache for next time
        model.save_to_cache(&path, &cache_path);

        model
    }
}

/// Try to load Model from cache if it exists and is more recent than all other files.
fn try_load_from_cache(dir_path: &Path, cache_path: &Path) -> Option<Model> {
    let cache_meta = std::fs::metadata(cache_path).ok()?;
    if !cache_meta.is_file() {
        return None;
    }
    let cache_mtime = cache_meta.modified().ok()?;

    // Check that cache is newer than all other files in the directory
    let entries = read_dir(dir_path).ok()?;
    for entry in entries.filter_map(Result::ok) {
        let entry_path = entry.path();
        if entry_path.file_name().map_or(true, |n| n == CACHE_FILE) {
            continue;
        }
        if let Ok(ft) = entry.file_type() {
            if ft.is_file() {
                if let Ok(entry_mtime) = entry.metadata().and_then(|m| m.modified()) {
                    if entry_mtime > cache_mtime {
                        return None; // A file is newer than cache, rebuild
                    }
                }
            }
        }
    }

    // Load from cache
    Model::from_cache(cache_path)
}

impl HasSubCommands for Model {
    fn get_command(&self, name: &str) -> Option<&Box<dyn Command>> {
        self.commands.iter().find(|command| command.name() == name)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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

    fn has_sub_commands(&self) -> bool;

    fn options(&self) -> &Vec<CommandOption>;

    fn args(&self) -> &Vec<CommandArg>;

    fn get_option(&self, name: &str) -> Option<&CommandOption> {
        self.options().iter().find(|option| option.name == name)
    }

    fn get_arg(&self, name: &str) -> Option<&CommandArg> {
        self.args().iter().find(|arg| arg.name == name)
    }
    fn get_path(&self) -> Option<&PathBuf>;

    /// Convert to serializable form for caching.
    fn as_serialized(&self) -> SerializedCommand;
}

/// A command that is located in a script file. The command may have sub-commands that are functions
/// in the script file.
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
    fn as_serialized(&self) -> SerializedCommand {
        SerializedCommand::Script {
            name: self.name.clone(),
            description: self.description.clone(),
            path: self.path.clone(),
            options: self.options.clone(),
            args: self.args.clone(),
            sub_commands: self
                .sub_commands
                .iter()
                .map(|c| c.as_serialized())
                .collect(),
        }
    }

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

    fn has_sub_commands(&self) -> bool {
        !self.sub_commands.is_empty()
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

    /// Create an EmbeddedCommand with sub_commands (used when deserializing from cache).
    pub fn with_sub_commands<S, T>(
        name: S,
        description: Option<T>,
        options: Vec<CommandOption>,
        args: Vec<CommandArg>,
        sub_commands: Vec<Box<dyn Command>>,
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
            sub_commands,
        }
    }
}

impl Command for EmbeddedCommand {
    fn as_serialized(&self) -> SerializedCommand {
        SerializedCommand::Embedded {
            name: self.name.clone(),
            description: self.description.clone(),
            options: self.options.clone(),
            args: self.args.clone(),
            sub_commands: self
                .sub_commands
                .iter()
                .map(|c| c.as_serialized())
                .collect(),
        }
    }

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

    fn has_sub_commands(&self) -> bool {
        !self.sub_commands.is_empty()
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
        std::fs::create_dir(&subdir_path).expect(
            format!(
                "Unable to create directory {}",
                subdir_path.to_str().unwrap()
            )
            .as_str(),
        );

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

    #[test]
    fn build_model_saves_to_cache() {
        let test_dir = tempfile::tempdir().unwrap();
        let script_path = test_dir.path().join("myscript.sh");
        File::create(&script_path)
            .unwrap()
            .write_all(b"# @name mycmd\n# @about A test command\n")
            .unwrap();

        // First build - creates cache
        let _ = super::Model::from(test_dir.path());
        let cache_path = test_dir.path().join(super::CACHE_FILE);
        assert!(cache_path.exists(), ".easy-cli cache should be created");

        let cached = super::Model::from_cache(&cache_path).unwrap();

        assert_eq!(
            cached.commands[0].name(),
            "mycmd",
            "cache file should contain the model"
        );
    }

    #[test]
    fn build_model_loads_from_cache_not_scripts() {
        let test_dir = tempfile::tempdir().unwrap();
        let script_path = test_dir.path().join("myscript.sh");
        File::create(&script_path)
            .unwrap()
            .write_all(b"# @name from_script\n# @about Script content\n")
            .unwrap();

        // Build once to create cache
        let _ = super::Model::from(test_dir.path());
        let cache_path = test_dir.path().join(super::CACHE_FILE);

        let other_model = super::Model::new(vec![Box::new(super::ScriptCommand::new(
            "from_cache".to_string(),
            Some("Script content".to_string()),
            "from_script".into(),
            vec![],
            vec![],
            vec![],
        ))]);

        other_model.save_to_cache(test_dir.path(), &cache_path);

        // Reload model - should come from cache (modified content), not from script
        let model = super::Model::from(test_dir.path());
        assert_eq!(
            model.commands[0].name(),
            "from_cache",
            "model should be loaded from cache; script still has 'from_script'"
        );
    }

    #[test]
    fn build_model_rebuilds_when_script_newer_than_cache() {
        let test_dir = tempfile::tempdir().unwrap();
        let script_path = test_dir.path().join("script.sh");
        File::create(&script_path)
            .unwrap()
            .write_all(b"# @name old\n")
            .unwrap();

        let _ = super::Model::from(test_dir.path());
        let cache_path = test_dir.path().join(super::CACHE_FILE);
        assert!(cache_path.exists());

        // Update script (make it newer than cache)
        std::thread::sleep(std::time::Duration::from_millis(10));
        File::create(&script_path)
            .unwrap()
            .write_all(b"# @name new\n")
            .unwrap();

        let model = super::Model::from(test_dir.path());
        assert_eq!(model.commands[0].name(), "new");

        let cache_path = test_dir.path().join(super::CACHE_FILE);

        // check that the cache has also been updated
        let cached = super::Model::from_cache(&cache_path).unwrap();
        assert_eq!(
            cached.commands[0].name(),
            "new",
            "model should be loaded from cache; script still has 'from_script'"
        );
    }
}
