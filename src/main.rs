#![allow(unused)]

use std::path::Path;

use crate::prelude::*;

mod commands;
mod prelude;
mod utils;

const DEFAULT_ERROR_MESSAGE: &'static str =
    "Usage: substitutor [check|gen|rev] or substitutor help to get more information.";

#[derive(Debug)]
struct ValidatedFile {
    format: String,
    file: File,
    name: String,
}

impl ValidatedFile {
    fn from_str(file_path: &str) -> Result<Self, Error> {
        let format = Path::new(&file_path)
            .extension()
            .and_then(|e| e.to_str())
            .ok_or(Error::InvalidFile(String::from(file_path)))?
            .to_owned();

        let file =
            File::open(&file_path).map_err(|_| Error::InvalidFile(String::from(file_path)))?;

        let name = file_path.to_owned();

        Ok(Self { format, file, name })
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
enum ValidCommands {
    Check,
    Generate,
    Reverse,
    Help,
}

#[derive(Debug)]
enum Error {
    NoCommand,
    InvalidCommand,
    NotEnoughArguments(ValidCommands),
    InvalidFile(String),
    InvalidIOFormat(String),
    InvalidFlag,
    HelpInvalidCommand,
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
        substitutor gen templateFile variableFile [optional flags]
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
        substitutor rev templateFile originalTheme [optional flags]
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

    let flags: Vec<_> = args
        .iter()
        .filter(|&x| x.starts_with("-"))
        .map(|x| x.to_string())
        .collect();

    let command = ValidCommands::from_str(&args[1])?;

    let command_args: Vec<_> = args
        .into_iter()
        .skip(2)
        .filter(|x| !x.starts_with("-"))
        .collect();

    match command {
        ValidCommands::Help if command_args.len() < 1 => Err(Error::NoCommand),
        ValidCommands::Help => {
            let help_command =
                ValidCommands::from_str(&command_args[0]).map_err(|_| Error::HelpInvalidCommand)?;
            Ok(commands::help(help_command))
        }
        (command) if command_args.len() < 2 => Err(Error::NotEnoughArguments(command)),
        ValidCommands::Check => {
            let file1 = ValidatedFile::from_str(&command_args[0])?;
            let file2 = ValidatedFile::from_str(&command_args[1])?;
            commands::check(file1, file2)
        }
        ValidCommands::Generate => todo!(),
        ValidCommands::Reverse => todo!(),
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
            error!(r#"Invalid file "{file_name}". Please check the file path and try again."#)
        }
        Err(Error::InvalidFlag) => error!("Invalid flag. Please check the flag and try again."),
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
    }
}
