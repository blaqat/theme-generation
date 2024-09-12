#![allow(unused)]
#![feature(let_chains)]
#![feature(if_let_guard)]

use std::path::Path;

use crate::prelude::*;

mod commands;
mod prelude;
mod utils;

const DEFAULT_ERROR_MESSAGE: &str =
    "Usage: substitutor [check|gen|rev] or substitutor help to get more information.";

#[derive(Debug)]
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
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum ValidCommands {
    Check,
    Generate,
    Reverse,
    Help,
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
            _ => Err(Error::InvalidCommand),
        }
    }

    fn list_commands() -> Vec<&'static str> {
        vec!["check", "gen", "rev", "help"]
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
        substitutor gen template_file variableFile [optional flags]
    Flags:
        -v	Toggles verbose logging for debug purposes
        -c originalTheme	Run substitutor check on originalTheme and generatedTheme
        -o directory	Set output directory of generatedTheme
        -n name	Set name of output theme file
Reverse:
    Description:
        - Template + OriginalTheme = Variables
        - This generates a variable file by substituting values in the original theme file with variables in the template file.
        - This takes the OriginalTheme as the source of truth. Things in the template that arent in the OriginalTheme will be ignored.
        - The generated file will be saved in the current directory.
    Usage:
        substitutor rev template_file originalTheme [optional flags]
    Flags:
        -v	Toggles verbose logging for debug purposes
        -c	Runs substitutor check on originalTheme and a generatedTheme of the generated variableFile
        -t int	Threshold for how many same colors to exist before adding to [colors] subgroup
        -o directory	Set output directory of variable file
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
            let mut directory = call_dir.clone();
            // d!(&flags);
            if flags.iter().any(|flag| flag.starts_with("-i")) {
                let flags = commands::generate::GenerateFlags::parse(flags.clone());
                directory = flags.directory();
                // d!(&directory);
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

            // d!(&template_file, variable_files);
            // todo!();

            commands::generate(template_file, variable_files, flags)
        }
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
