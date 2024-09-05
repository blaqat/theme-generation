use crate::prelude::*;
use std::path::PathBuf;

// Reverse:
//     Description:
//         - Template + OriginalTheme = Variables
//         - This generates a variable file by substituting values in the original theme file with variables in the template file.
//         - This takes the OriginalTheme as the source of truth. Things in the template that arent in the OriginalTheme will be ignored.
//         - The generated file will be saved in the current directory.
//     Usage:
//         substitutor rev template_file originalTheme [optional flags]
//     Flags:
//         -v	Toggles verbose logging for debug purposes
//         -c	Runs substitutor check on originalTheme and a generatedTheme of the generated variableFile
//         -t int	Threshold for how many same colors to exist before adding to [colors] subgroup
//         (-t=N)
//         -o directory	Set output directory of variable file

#[derive(PartialEq, Debug)]
enum ReverseFlags {
    Verbose,
    Check,
    Threshold(i32),
    OutputDirectory(PathBuf),
}

impl ReverseFlags {
    fn from_str(flag: &str) -> Result<Self, Error> {
        match flag {
            "-v" => Ok(Self::Verbose),
            "-c" => Ok(Self::Check),
            flag if flag.starts_with("-o") => {
                let path = flag.split("=").last().unwrap();
                let path = Path::new(path);
                if !path.exists() {
                    return Err(Error::InvalidFlag("reverse".to_owned(), flag.to_owned()));
                }
                Ok(Self::OutputDirectory(path.to_path_buf()))
            }
            flag if flag.starts_with("-t") => {
                let threshold = flag.split("=").last().unwrap();
                let threshold = threshold
                    .parse()
                    .map_err(|_| Error::InvalidFlag("reverse".to_owned(), flag.to_owned()))?;
                Ok(Self::Threshold(threshold))
            }
            _ => Err(Error::InvalidFlag("reverse".to_owned(), flag.to_owned())),
        }
    }

    fn parse(flags: Vec<String>) -> Result<Vec<Self>, Error> {
        flags.iter().map(|flag| Self::from_str(flag)).collect()
    }
}

// enum ValueType {
//     Null,
//     Color(String),
//     String(String),
//     Number(i32|float),
//     Boolean(bool),
//     Array(Vec<Value>),
//     Object(HashMap<String, Value>),
// }

// impl ValueType {
//     fn from_value(value: Value) -> Self {
//         match value {
//             Value::Null => Self::Null,
//             Value::String(s) => Self::String(s),
//             Value::Number(n) => Self::Number(n),
//             Value::Bool(b) => Self::Boolean(b),
//             Value::Array(a) => Self::Array(a),
//             Value::Object(o) => Self::Object(o),
//         }
//     }
// }

// struct SourcedValue {
//     value: Value,
//     path: String,
// }

pub fn reverse(
    template: ValidatedFile,
    theme: ValidatedFile,
    flags: Vec<String>,
) -> Result<(), Error> {
    p!(
        "Template:\n{:?}\n\nTheme:\n{:?}\n\nFlags:\n{:?}",
        template,
        theme,
        ReverseFlags::parse(flags)?
    );
    todo!()
    // Step 1: Deserialize the template and theme files into Objects.
    /* Step 2: Create new Objects that will be used to store
        - Variables HashMap<String, LinkedValue>
        - Colors HashMap<String, Vec<String>>
        - Deletions
        - Overrides
    */
    // Step 3: Deep search through the template file for all keys not in the theme file and add them to the deletions HashMap.
    //
}
