use crate::prelude::*;
use commands::reverse::json::JsonPath;
use std::{cell::RefCell, cmp::Ordering, io::Read, path::PathBuf, ptr::replace};

// Generate:
//     Description:
//         - Template + Variables = GeneratedTheme
//         - This generates a new file by substituting variables in the template file with values from the variable file.
//         - This takes the Template as the source of truth. Things in the variable file that arent in the template will be ignored.
//         - The generated file will be saved in the current directory.
//     Usage:
//         substitutor gen template_file variableFile [optional flags]
//     Flags:
//         -v	Toggles verbose logging for debug purposes
//         -c originalTheme	Run substitutor check on originalTheme and generatedTheme
//         -o directory	Set output directory of generatedTheme
//         -n name	Set name of output theme file

#[derive(PartialEq, Debug)]
enum GenerateFlags {
    Verbose,
    Check,
    OutputDirectory(PathBuf),
    Name(String),
}

#[derive(PartialEq, Debug)]
struct Flags {
    verbose: bool,
    check: bool,
    output_directory: PathBuf, // Default to current directory
    name: String,
}

impl GenerateFlags {
    fn into_vec(flags: Vec<String>) -> Result<Vec<Self>, Error> {
        flags.iter().map(|flag| Self::from_str(flag)).collect()
    }

    fn parse(flags: Vec<String>) -> Flags {
        let flags = Self::into_vec(flags).unwrap();
        let mut verbose = false;
        let mut check = false;
        let mut output_directory = PathBuf::from(".");
        let mut name = String::from("generated-theme");

        for flag in flags {
            match flag {
                Self::Verbose => verbose = true,
                Self::Check => check = true,
                Self::OutputDirectory(path) => output_directory = path,
                Self::Name(n) => name = n,
            }
        }

        Flags {
            verbose,
            check,
            output_directory,
            name,
        }
    }
}

impl FromStr for GenerateFlags {
    type Err = Error;

    fn from_str(flag: &str) -> Result<Self, Error> {
        match flag {
            "-v" => Ok(Self::Verbose),
            "-c" => Ok(Self::Check),
            flag if flag.starts_with("-n") => {
                let name = flag.split("=").last().unwrap();
                Ok(Self::Name(name.to_owned()))
            }
            flag if flag.starts_with("-o") => {
                let path = flag.split("=").last().unwrap();
                let path = Path::new(path);
                if !path.exists() {
                    return Err(Error::InvalidFlag("reverse".to_owned(), flag.to_owned()));
                }
                Ok(Self::OutputDirectory(path.to_path_buf()))
            }
            _ => Err(Error::InvalidFlag("reverse".to_owned(), flag.to_owned())),
        }
    }
}

mod steps {
    use super::*;
    use commands::reverse::json::JsonPath;
    use commands::reverse::variable::{ParsedValue, ParsedVariable};
    use serde_json::json;
    type Value = serde_json::Value;

    pub fn resolve_variables(
        resolving: &Value,
        _source: &Value,
        _operations: &Vec<Vec<ColorChange>>,
    ) -> Value {
        let mut resolved: Value = json!({});
        match resolving {
            Value::Object(obj) => {
                for (key, value) in obj.iter() {
                    // d!(key);
                    resolved[key] = resolve_variables(value, _source, _operations);
                }
            }
            Value::Array(arr) => {
                let mut res_arr = Vec::with_capacity(arr.len());
                for (i, value) in arr.iter().enumerate() {
                    res_arr.push(resolve_variables(value, _source, _operations));
                }
                resolved = Value::Array(res_arr);
            }

            Value::String(str) if let Ok(parsed) = str.parse::<ParsedValue>() => {
                // d!(&parsed);
                match parsed {
                    ParsedValue::Variables(ref var)
                        if let Ok(parsed_var) = var.first().unwrap().parse::<ParsedVariable>() =>
                    {
                        let path = parsed_var
                            .name
                            .replace(".", "/")
                            .parse::<JsonPath>()
                            .unwrap();

                        // d!(&path);

                        let value = path.traverse(_source);

                        if let Ok(v) = value {
                            let mut new_ops = _operations.clone();
                            new_ops.push(parsed_var.operations);
                            resolved = resolve_variables(v, _source, &new_ops);
                        } else {
                            resolved = Value::Null;
                            // d!(&resolved.clone());
                        }

                        // d!(&resolved);
                    }
                    ParsedValue::Color(ref c) => {
                        let mut c = c.clone();
                        // d!(_operations);
                        // d!(&c);
                        c.update_ops(_operations.as_slice());
                        // d!(&c);
                        resolved = Value::String(c.to_string());
                        // resolved = parsed.into_value();
                    }
                    ParsedValue::Null => unreachable!(),
                    v => resolved = v.into_value(),
                }
            }

            value => {
                resolved = value.clone();
            }
        }

        resolved
    }

    // fn match_rec(a: &Value, variables: &Value) -> Value {

    // }

    pub fn match_variables(template: &Value, variables: &Value) -> Value {
        let mut new_data = template.clone();

        for (key, value) in template.as_object().unwrap().iter() {
            match value {
                Value::String(str) if let Ok(parsed) = str.parse::<ParsedValue>() => match parsed {
                    ParsedValue::Variables(ref var)
                        if let Ok(parsed_var) = var.first().unwrap().parse::<ParsedVariable>()
                            && let Ok(path) =
                                parsed_var.name.replace(".", "/").parse::<JsonPath>() =>
                    {
                        new_data[key] = path.traverse(variables).unwrap_or(&Value::Null).clone();
                        // d!(&new_data[key]);
                    }
                    v => new_data[key] = v.into_value(),
                },

                serde_json::Value::Array(a) => {
                    let mut new_arr = Vec::with_capacity(a.len());
                    for (i, value) in a.iter().enumerate() {
                        new_arr.push(match_variables(value, variables));
                    }
                    new_data[key] = Value::Array(new_arr);
                }
                serde_json::Value::Object(o) => {
                    new_data[key] = match_variables(value, variables);
                }

                v => new_data[key] = v.clone(),
            }
        }

        new_data
    }
}

pub fn generate(
    template: ValidatedFile,
    mut variables: ValidatedFile,
    flags: Vec<String>,
) -> Result<(), Error> {
    // p!(
    //     "Template:\n{:?}\n\nToml:\n{:?}\n\nFlags:\n{:?}",
    //     template,
    //     variables,
    //     GenerateFlags::into_vec(flags)?
    // );

    let flags = GenerateFlags::parse(flags);

    // Step 1: Deserialize the template and variable files into Objects.
    let mut template: serde_json::Value = serde_json::from_reader(&template.file)
        .map_err(|json_err| Error::Processing(format!("Invalid template json: {}", json_err)))?;
    let variables: serde_json::Value = {
        let mut contents = String::new();
        variables
            .file
            .read_to_string(&mut contents)
            .map_err(|json_err| {
                Error::Processing(format!("Invalid variable toml: {}", json_err))
            })?;
        serde_json::to_value(toml::from_str::<toml::Value>(&contents).unwrap())
    }
    .map_err(|json_err| Error::Processing(format!("Invalid variable toml: {}", json_err)))?;

    // d!(template, variables);

    // Step 2: Resolve recursive variables
    let variables = steps::resolve_variables(&variables, &variables, &vec![]);
    // d!(&variables);

    // Step 4: Apply Deletions
    if let Some(del_obj) = variables.get("deletions")
        && let Some(deletions) = del_obj.as_object().unwrap().get("keys")
    {
        let deletions = deletions.as_array().unwrap();
        for key in deletions {
            let path = key.as_str().unwrap().parse::<JsonPath>().map_err(|_| {
                Error::Processing(format!(
                    "Invalid path in deletions: {}",
                    key.as_str().unwrap()
                ))
            })?;

            let found = path.remove(&mut template);
        }
    }

    // Step 3: Match variables with template
    let mut matches = steps::match_variables(&template, &variables);
    // d!(&matches);

    // Step 5: Apply Overrides
    if let Some(overrides) = variables.get("overrides") {
        let overrides = overrides.as_object().unwrap();
        for (key, value) in overrides.iter() {
            let path = key
                .parse::<JsonPath>()
                .map_err(|_| Error::Processing(format!("Invalid path in overrides: {}", key)))?;
            path.pave(&mut matches, value.clone());
        }
    }

    // Step 6: Generate the new theme file
    let json_output = serde_json::to_string_pretty(&matches).unwrap();
    let default_name = "/name".parse::<JsonPath>().unwrap().traverse(&matches).ok();

    let file_name = format!("{}.json", {
        if flags.name == "generated-theme"
            && let Some(default) = default_name
        {
            default.as_str().unwrap()
        } else {
            &flags.name
        }
    });

    let out_dir = flags.output_directory;

    let mut out_file = out_dir.clone();
    out_file.push(file_name);

    let mut file = File::create(out_file)
        .map_err(|e| Error::Processing(format!("Could not create file: {}", e)))?;
    file.write_all(json_output.as_bytes());

    // d!(&matches);

    Ok(())
    // todo!()
}
