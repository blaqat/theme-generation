#![allow(unused)]
#![feature(let_chains)]
#![feature(if_let_guard)]

use commands::variable;

use crate::prelude::*;

mod commands;
mod prelude;
mod utils;

const DEFAULT_ERROR_MESSAGE: &str =
    "Usage: substitutor [check|gen|rev] or substitutor help to get more information.";

#[derive(Debug, Clone)]
enum FileType {
    Theme,
    Template,
    Variable,
}

#[derive(Debug)]
struct ValidatedFile {
    format: String,
    file: File,
    name: String,
    file_type: FileType,
}

impl ValidatedFile {
    fn clone(&self) -> Self {
        let new_file = File::open(&self.name).unwrap();
        Self {
            format: self.format.clone(),
            file: new_file,
            name: self.name.clone(),
            file_type: self.file_type.clone(),
        }
    }

    fn from_str(file_path: &str) -> Result<Self, Error> {
        let format = Path::new(&file_path)
            .extension()
            .and_then(|e| e.to_str())
            .ok_or(Error::InvalidFile(String::from(file_path)))?
            .to_owned();

        let file_type = match format.as_str() {
            "json" => FileType::Theme,
            "toml" => FileType::Variable,
            "template" => FileType::Template,
            _ => return Err(Error::InvalidIOFormat(format)),
        };

        let file =
            File::open(file_path).map_err(|_| Error::InvalidFile(String::from(file_path)))?;

        let name = file_path.to_owned();

        Ok(Self {
            format,
            file,
            name,
            file_type,
        })
    }

    fn all_variable_files(source_directory: &Path) -> Result<Vec<Self>, Error> {
        // Variable files are toml files.
        let mut files = Vec::new();

        for entry in source_directory
            .read_dir()
            .map_err(|_| Error::InvalidFile(String::from(source_directory.to_str().unwrap())))?
        {
            // d!(&entry);
            let entry = entry.map_err(|_| {
                Error::InvalidFile(String::from(source_directory.to_str().unwrap()))
            })?;
            let path = entry.path();
            let path_str = path
                .to_str()
                .ok_or(Error::InvalidFile(String::from(path.to_str().unwrap())))?;
            if path.is_file() && path_str.ends_with(".toml") {
                files.push(ValidatedFile::from_str(path_str)?);
            }
        }

        Ok(files)
    }

    fn from_file(file: File, path: &Path) -> Result<Self, Error> {
        let format = path
            .extension()
            .and_then(|e| e.to_str())
            .ok_or(Error::InvalidFile(String::from(path.to_str().unwrap())))?
            .to_owned();

        let file_type = match format.as_str() {
            "json" => FileType::Theme,
            "toml" => FileType::Variable,
            "template" => FileType::Template,
            _ => return Err(Error::InvalidIOFormat(format)),
        };

        let name = path.to_str().unwrap().to_owned();

        Ok(Self {
            format,
            file,
            name,
            file_type,
        })
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum ValidCommands {
    Check,
    Generate,
    Reverse,
    Help,
    Watch,
    Edit,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Error {
    NoCommand,
    InvalidCommand,
    NotEnoughArguments(ValidCommands),
    InvalidFile(String),
    InvalidFileType,
    InvalidFlag(String, String),
    InvalidIOFormat(String),
    HelpInvalidCommand,
    Processing(String),
}

impl ValidCommands {
    fn from_str(command: &str) -> Result<Self, Error> {
        match command {
            "check" => Ok(Self::Check),
            "gen" => Ok(Self::Generate),
            "rev" => Ok(Self::Reverse),
            "help" => Ok(Self::Help),
            "watch" => Ok(Self::Watch),
            "edit" => Ok(Self::Edit),
            _ => Err(Error::InvalidCommand),
        }
    }

    fn list_commands() -> Vec<&'static str> {
        vec!["check", "gen", "rev", "help", "watch", "edit"]
    }
}

/*
Substitutor Program:
Check:
    Description:
        - This checks line by line if the original file and the new file are the same.
        - Displays similarity metrics.
        - Will help in debugging issues in generation/reverse process.
            - Template + Variables = GeneratedTheme == OriginalTheme
    Usage:
        substitutor check originalFile newFile
Generate:
    Description:
        - Template + Variables = GeneratedTheme
        - This generates a new file by substituting variables in the template file with values from the variable file.
        - This takes the Template as the source of truth. Things in the variable file that arent in the template will be ignored.
        - The generated file will be saved in the current directory.
    Usage:
        substitutor gen template_file variableFile|all [optional flags]
    Flags:
        -o directory	Set output directory of generatedTheme
        -n name	Set name of output theme file
        -p path    Inner path to the theme in the template file
        -i directory    Set directory where the .toml files are located
Reverse:
    Description:
        - Template + OriginalTheme = Variables
        - This generates a variable file by substituting values in the original theme file with variables in the template file.
        - This takes the OriginalTheme as the source of truth. Things in the template that arent in the OriginalTheme will be ignored.
        - The generated file will be saved in the current directory.
    Usage:
        substitutor rev template_file originalTheme [optional flags]
    Flags:
        -t int	Threshold for how many same colors to exist before adding to [colors] subgroup
        -o directory	Set output directory of variable file
        -p path    Inner path to the theme in the template file
Watch Mode:
    Description:
        - Watch changes to .toml files in a directory or a specific file and generate the theme file on each change.
        - This makes it better to see live changes fast as you are making a theme
    Usage:
        substitutor watch templateFile variableFile|all [optional flags]
    Flags:
        -p path    Inner path to the theme in the theme file
        -o directory	Set output directory of generatedTheme
        -n name	Set name of output theme file
        -i directory    Set directory where the .toml files are located
Edit Mode:
    Description:
        - Make a directory in a pretetermined spot e.g. $HOME/.config/substitutor
            - If the directory is not empty, prompt user to continue edit, save edit, or delete and start over.
        - Automatically run `substitor watch templateFile all [flags]` in the directory.
        - This makes it way faster to get started editing rather than having to reverse and then generate manually, this does both.
    Usage:
        substitutor edit themeFile templateFile [watch flags]
    Flags: (Same as watch flags)
*/

fn run(args: Vec<String>) -> Result<(), Error> {
    if args.len() < 2 {
        return Err(Error::NoCommand);
    }

    let call_dir = std::env::current_dir().unwrap();

    let mut flags: Vec<_> = args
        .iter()
        .filter(|&x| x.starts_with("-"))
        .map(|x| x.to_string())
        .collect();

    flags.sort();
    flags.dedup();

    let command = ValidCommands::from_str(&args[1])?;

    let command_args: Vec<_> = args
        .into_iter()
        .skip(2)
        .filter(|x| !x.starts_with("-"))
        .collect();

    let get_generation_files = || -> Result<(PathBuf, ValidatedFile, Vec<ValidatedFile>), Error> {
        let mut directory = call_dir.clone();
        if flags.iter().any(|flag| flag.starts_with("-i")) {
            let flags = commands::generate::GenerateFlags::parse(flags.clone());
            directory = flags.directory();
        }

        let (template_file, variable_files) =
            match (command_args[0].as_str(), command_args[1].as_str()) {
                ("all", template_file) | (template_file, "all") => {
                    let template_file = ValidatedFile::from_str(template_file)?;
                    if let FileType::Template = template_file.file_type {
                        let variable_files = ValidatedFile::all_variable_files(&directory)?;
                        (template_file, variable_files)
                    } else {
                        return Err(Error::InvalidFileType);
                    }
                }
                (template_file, variable_file) => {
                    let files = (
                        ValidatedFile::from_str(template_file)?,
                        ValidatedFile::from_str(variable_file)?,
                    );

                    match (&files.0.file_type, &files.1.file_type) {
                        (FileType::Template, FileType::Variable) => (files.0, vec![files.1]),
                        (FileType::Variable, FileType::Template) => (files.1, vec![files.0]),
                        _ => return Err(Error::InvalidFileType),
                    }
                }
            };

        Ok((directory, template_file, variable_files))
    };

    match command {
        ValidCommands::Help if command_args.is_empty() => Err(Error::NoCommand),
        ValidCommands::Help => {
            let help_command =
                ValidCommands::from_str(&command_args[0]).map_err(|_| Error::HelpInvalidCommand)?;
            commands::help(help_command);
            Ok(())
        }
        (command) if command_args.len() < 2 => Err(Error::NotEnoughArguments(command)),
        ValidCommands::Check => {
            let file1 = ValidatedFile::from_str(&command_args[0])?;
            let file2 = ValidatedFile::from_str(&command_args[1])?;
            commands::check(file1, file2)
        }
        ValidCommands::Reverse => {
            let template_file = ValidatedFile::from_str(&command_args[0])?;
            let theme_file = ValidatedFile::from_str(&command_args[1])?;

            match (&template_file.file_type, &theme_file.file_type) {
                (FileType::Template, FileType::Theme) => {
                    commands::reverse(template_file, theme_file, flags)
                }
                (FileType::Theme, FileType::Template) => {
                    commands::reverse(theme_file, template_file, flags)
                }
                _ => Err(Error::InvalidFileType),
            }
        }
        ValidCommands::Generate => {
            let (_, template_file, variable_files) = get_generation_files()?;

            commands::generate(template_file, variable_files, flags)
        }

        ValidCommands::Watch => {
            // substitutor gen $"($template_file)" all -n=$"($out_name)" -p=/themes -o=~/.config/zed/themes/
            let (mut directory, template_file, variable_files) = get_generation_files()?;

            // d!(&directory);
            let (tx, rx) = std::sync::mpsc::channel();
            let mut debouncer = new_debouncer(std::time::Duration::from_millis(100), tx)
                .map_err(|e| Error::Processing(String::from("Error creating notify watcher.")))?;

            let mut watcher = debouncer.watcher();

            // d!(variable_files);

            for file in &variable_files {
                let mut path = directory.clone();
                path.push(&file.name);
                // d!(&path);

                watcher
                    .watch(&path, RecursiveMode::Recursive)
                    .map_err(|e| Error::Processing(String::from("Error watching file.")))?;
            }

            loop {
                match rx.try_recv() {
                    Ok(ref event) if let Ok(ref event) = event => {
                        let variable_files = variable_files.iter().map(|v| v.clone()).collect();
                        commands::generate(template_file.clone(), variable_files, flags.clone())?;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        // No events to process, continue the loop
                    }
                    Ok(_) => {}
                    Err(e) => {
                        error!("watch error: {:?}", e);
                    }
                }
            }

            Ok(())
        }

        ValidCommands::Edit => todo!(),
    }
}

fn main() {
    let args: Vec<String> = args().collect();

    match run(args) {
        Ok(_) => (),
        Err(Error::NoCommand) => error!("{}", DEFAULT_ERROR_MESSAGE),
        Err(Error::InvalidCommand) => {
            error!(
                "Invalid command. Please use one of the following: {:?}",
                ValidCommands::list_commands()
            )
        }
        Err(Error::InvalidFile(file_name)) => {
            error!(r#""{file_name}" is not a file. Please check the file path and try again."#)
        }
        Err(Error::InvalidFileType) => {
            error!(r#"Invalid types for files provided. Please check the usage."#)
        }
        Err(Error::InvalidFlag(command, flag)) => {
            error!(r#"Invalid flag "{flag}" for the "{command}" command. Please check the usage."#)
        }
        Err(Error::HelpInvalidCommand) => {
            error!(
                "Invalid command argument for help. Please use one of the following: {:?}",
                ValidCommands::list_commands()
            )
        }
        Err(Error::NotEnoughArguments(command)) => {
            error!(
                "Not enough arguments for the {:?} command. Please check the usage:",
                command
            );
            commands::help(command)
        }
        Err(Error::InvalidIOFormat(format)) => {
            error!(
                r#"Unhandeled file format "{format}". Please make an issue to start future support"#
            )
        }
        Err(Error::Processing(message)) => {
            error!(r#"An error occured while processing: "{}""#, message)
        }
    }
}
