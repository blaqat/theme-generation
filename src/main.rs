#![feature(let_chains)]
#![feature(if_let_guard)]
#![warn(clippy::all, clippy::pedantic, clippy::nursery)]
use crate::prelude::*;

mod commands;
mod prelude;
mod utils;

/**
# Substitutor Program:
# Check:
    - This checks line by line if the original file and the new file are the same.
    - Displays similarity metrics.
    - Will help in debugging issues in generation/reverse process.
        - Template + Variables = GeneratedTheme == OriginalTheme
### Usage:
    `substitutor check originalFile newFile`


# Generate:
    - Template + Variables = GeneratedTheme
    - This generates a new file by substituting variables in the template file with values from the variable file.
    - This takes the Template as the source of truth. Things in the variable file that arent in the template will be ignored.
    - The generated file will be saved in the current directory.
### Usage:
    `substitutor gen template_file variableFile [optional flags]`
### Flags:
    -o directory    Set output directory of variable file
    -i directory    Set directory where the .toml files are located
    -p path         Json Path to start the reverse process at
    -n              Name of the output file
    -r              Overwrite the output file of the same name if it exists

# Reverse:
    - Template + OriginalTheme = Variables
    - This generates a variable file by substituting values in the original theme file with variables in the template file.
    - This takes the OriginalTheme as the source of truth. Things in the template that arent in the OriginalTheme will be ignored.
    - The generated file will be saved in the current directory.
### Usage:
    `substitutor rev template_file originalTheme [optional flags]`
### Flags:
    -t int          Threshold for how many same colors to exist before adding to [colors] subgroup
    -o directory    Set output directory of variable file
    -n              Name of the output file
    -p path         Json Path to start the reverse process at

# Watch Mode:
    - Watch changes to .toml files in a directory or a specific file and generate the theme file on each change.
    - This makes it better to see live changes fast as you are making a theme
### Usage:
    `substitutor watch templateFile variableFile|all [optional flags]`
### Flags:
    -p path         Inner path to the theme in the theme file
    -o directory    Set output directory of generatedTheme
    -n name         Set name of output theme file
    -i directory    Set directory where the .toml files are located

# Edit Mode:
    - Make a directory in a pretetermined spot e.g. $HOME/.config/substitutor
        - If the directory is not empty, prompt user to continue edit, save edit, or delete and start over.
    - Automatically run `substitor watch templateFile all [flags]` in the directory.
    - This makes it way faster to get started editing rather than having to reverse and then generate manually, this does both.
### Usage:
    `substitutor edit themeFile templateFile [watch flags]`
### Flags: (Same as watch flags)
*/
fn main() {
    let args: Vec<String> = args().collect();

    match run_command(args) {
        Ok(()) => (),
        Err(ProgramError::NoCommand) => {
            error!(
                "Usage: substitutor [{}] or substitutor help to get more information.",
                ValidCommands::list_commands().join("|")
            );
        }
        Err(ProgramError::InvalidCommand) => {
            error!(
                "Invalid command. Please use one of the following: {:?}",
                ValidCommands::list_commands()
            );
        }
        Err(ProgramError::InvalidFile(file_name)) => {
            error!(r#""{file_name}" is not a file. Please check the file path and try again."#);
        }
        Err(ProgramError::InvalidFileType) => {
            error!(r"Invalid types for files provided. Please check the usage.");
        }
        Err(ProgramError::InvalidFlag(command, flag)) => {
            error!(r#"Invalid flag "{flag}" for the "{command}" command. Please check the usage."#);
        }
        Err(ProgramError::HelpInvalidCommand) => {
            error!(
                "Invalid command argument for help. Please use one of the following: {:?}",
                ValidCommands::list_commands()
            );
        }
        Err(ProgramError::NotEnoughArguments(command)) => {
            error!(
                "Not enough arguments for the {:?} command. Please check the usage:",
                command
            );
            commands::help(&command);
        }
        Err(ProgramError::InvalidIOFormat(format)) => {
            error!(
                r#"Unhandeled file format "{format}". Please make an issue to start future support"#
            );
        }
        Err(ProgramError::Processing(message)) => {
            error!("{message}");
        }
        Err(ProgramError::HelpAll) => {
            println!("---- NEW ----");
            commands::help(&ValidCommands::New);
            println!("---- WATCH ----");
            commands::help(&ValidCommands::Watch);
            println!("---- REVERSE ----");
            commands::help(&ValidCommands::Reverse);
            println!("---- CHECK ----");
            commands::help(&ValidCommands::Check);
            println!("---- GENERATE ----");
            commands::help(&ValidCommands::Generate);
            println!("---- EDIT ----");
            commands::help(&ValidCommands::Edit);
        }
    }
}
