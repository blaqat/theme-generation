use crate::prelude::*;
use commands::check::{json_deep_diff, DiffInfo};
use core::fmt;
use std::path::PathBuf;
use tokio::io::AsyncBufReadExt;
// use json_patch;
use serde_json::{json, Map, Value};

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

#[derive(PartialEq, Debug)]
enum ParsedString {
    String(String),
    Color(Color),
    Variable(Vec<String>),
}

impl fmt::Display for ParsedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::String(s) => write!(f, r#"Str"{}""#, s),
            Self::Color(c) => write!(f, "Color({})", c),
            Self::Variable(v) => write!(f, "Vars[${}]", v.join(", $")),
        }
    }
}

impl std::str::FromStr for ParsedString {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("$") || s.starts_with("@") {
            let vars = s
                .split("|")
                .map(str::trim)
                .filter_map(|var| {
                    if var.starts_with("$") {
                        Some(var.chars().skip(1).collect())
                    } else if var.starts_with("@") {
                        var.get(1..).map(|s| format!("color.{}", s))
                    } else {
                        None
                    }
                })
                .collect();
            Ok(Self::Variable(vars))
        } else if let Ok(color) = s.parse() {
            Ok(Self::Color(color))
        } else {
            Ok(Self::String(s.to_owned()))
        }
    }
}

#[derive(Debug)]
struct KeyDiffInfo {
    missing: Vec<String>,
    collisions: Vec<String>,
}

impl KeyDiffInfo {
    fn extend(&mut self, other: Self) {
        self.missing.extend(other.missing);
        self.collisions.extend(other.collisions);
    }
}

fn key_diff(data1: &Value, data2: &Value, prefix: String) -> KeyDiffInfo {
    let mut info = KeyDiffInfo {
        missing: Vec::new(),
        collisions: Vec::new(),
    };

    match (data1, data2) {
        (Value::Object(map1), Value::Object(map2)) => {
            for (key, val) in map1.iter() {
                match map2.get(key) {
                    Some(val2) => {
                        let next_diff = key_diff(val, val2, format!("{prefix}/{key}"));
                        info.extend(next_diff)
                    }
                    _ => info.missing.push(format!("{prefix}/{key}")),
                }
            }
        }
        (Value::Array(vec1), Value::Array(vec2)) => {
            for (key, val) in vec1.iter().enumerate() {
                match vec2.get(key) {
                    Some(val2) => {
                        let next_diff = key_diff(val, val2, format!("{prefix}[{key}]"));
                        info.extend(next_diff);
                    }
                    _ => info.missing.push(format!("{prefix}[{key}]")),
                }
            }
        }
        (val1, val2) if has_keys(val1) != has_keys(val2) => {
            info.collisions.push(prefix);
        }

        (Value::String(str1), Value::String(str2)) => {
            p!(
                "STR: {} {}",
                str1.parse::<ParsedString>().unwrap(),
                str2.parse::<ParsedString>().unwrap()
            )
        }
        _ => (),
    }

    info
}

struct SourcedValue {
    value: Value,
    path: String,
}

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
    // Step 1: Deserialize the template and theme files into Objects.
    let theme: Value = serde_json::from_reader(&theme.file)
        .map_err(|json_err| Error::Processing(format!("Invalid theme json: {}", json_err)))?;
    let template: Value = serde_json::from_reader(&template.file)
        .map_err(|json_err| Error::Processing(format!("Invalid template json: {}", json_err)))?;
    // .map_err(|e| Error::Processing(String::from("Invalid template json.")))?;

    // Step 2: Built Data Structures (Deletions, Overrides, Variables, Colors)
    // let mut deletions: HashMap<String, Value> = get_deletions(&theme, &template);
    let deletion_diff = key_diff(&template, &theme, String::from(""));
    let override_diff = key_diff(&theme, &template, String::from(""));

    p!("Deletions:\n{:?} \n\nOverrides:\n{:?}", deletion_diff, override_diff);

    todo!()
}
