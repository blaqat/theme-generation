use crate::{prelude::*, ValidCommands};

pub fn help(command: ValidCommands) {
    let help_text = match command {
        ValidCommands::Check => "Description:
    - This checks line by line if the original file and the new file are the same.
    - Displays similarity metrics.
    - Will help in debugging issues in generation/reverse process.
    - Template + Variables = GeneratedTheme == OriginalTheme

Usage:
    substitutor check originalFile newFile
",
        ValidCommands::Generate => "Description:
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
        ",
        ValidCommands::Reverse => "Description:
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
        ",
        ValidCommands::Help => "Displays help information.",
    };

    p!("{help_text}");
}
