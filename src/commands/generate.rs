/**
Generate:
    Description:
        - Template + Variables = `GeneratedTheme`
        - This generates a new file by substituting variables in the template file with values from the variable file.
        - This takes the Template as the source of truth. Things in the variable file that arent in the template will be ignored.
        - The generated file will be saved in the current directory.
    Usage:
        substitutor gen `template_file` variableFile [optional flags]
    Flags:
        -o directory    Set output directory of variable file
        -i directory    Set directory where the .toml files are located
        -p path         Json Path to start the reverse process at
        -n              Name of the output file
        -r              Overwrite the output file of the same name if it exists
*/
use crate::prelude::*;
use regex::Regex;
use std::{io::Read, path::PathBuf};

pub const VALID_FLAGS: [&str; 5] = ["-o", "-i", "-p", "-n", "-r"];

#[derive(Debug)]
pub enum FlagTypes {
    OutputDirectory(PathBuf),
    InputDirectory(PathBuf),
    InnerPath(JSPath),
    Name(String),
    ReplaceName,
}

#[derive(Debug)]
pub struct Flags {
    replace_name: bool,
    output_directory: PathBuf, // Default to current directory
    input_directory: PathBuf,  // Default to current directory
    name: String,
    path: Option<JSPath>,
}

impl Flags {
    pub fn directory(&self) -> PathBuf {
        self.input_directory.clone()
    }
}

impl FlagTypes {
    fn into_vec(flags: &[String]) -> Result<Vec<Self>, ProgramError> {
        flags.iter().map(|flag| Self::from_str(flag)).collect()
    }

    pub fn parse(flags: &[String]) -> Result<Flags, ProgramError> {
        let flags = Self::into_vec(flags)?;
        let mut output_directory = PathBuf::from(".");
        let mut input_directory = PathBuf::from(".");
        let mut name = String::from("generated-theme");
        let mut path = None;
        let mut replace_name = false;

        for flag in flags {
            match flag {
                Self::OutputDirectory(path) => output_directory = path,
                Self::InputDirectory(path) => input_directory = path,
                Self::Name(n) => name = n,
                Self::InnerPath(p) => path = Some(p),
                Self::ReplaceName => replace_name = true,
            }
        }

        Ok(Flags {
            replace_name,
            output_directory,
            input_directory,
            name,
            path,
        })
    }
}

impl FromStr for FlagTypes {
    type Err = ProgramError;

    fn from_str(flag: &str) -> Result<Self, ProgramError> {
        let get_directory = |path: &str| -> Result<PathBuf, ProgramError> {
            let path = path.replace('~', std::env::var("HOME").unwrap().as_str());
            let path = Path::new(&path);
            if !path.exists() {
                return Err(ProgramError::Processing(format!(
                    "Directory does not exist: {}",
                    path.to_str().unwrap()
                )));
            }
            Ok(path.to_path_buf())
        };
        match flag {
            "-r" => Ok(Self::ReplaceName),
            flag if flag.starts_with("-n") => {
                let name = flag.split('=').next_back().unwrap();
                Ok(Self::Name(name.to_owned()))
            }
            flag if flag.starts_with("-p") => {
                let path = flag.split('=').next_back().unwrap();
                let path = JSPath::from_str(path).map_err(|_| {
                    ProgramError::InvalidFlag("reverse".to_owned(), flag.to_owned())
                })?;
                Ok(Self::InnerPath(path))
            }
            flag if flag.starts_with("-i") => {
                let path = flag.split('=').next_back().unwrap();
                get_directory(path).map(Self::InputDirectory)
            }
            flag if flag.starts_with("-o") => {
                let path = flag.split('=').next_back().unwrap();
                get_directory(path).map(Self::OutputDirectory)
            }
            _ => Err(ProgramError::InvalidFlag(
                "reverse".to_owned(),
                flag.to_owned(),
            )),
        }
    }
}

mod steps {
    use super::{JSPath, Map, Operation, ParsedValue, ParsedVariable, ProgramError, Regex};
    use crate::error;
    use serde_json::json;
    type Value = serde_json::Value;
    const MAX_RECURSION_DEPTH: usize = 8;

    pub fn resolve_self_variables(source: &Value, key: &Vec<&str>, max_depth: usize) -> Value {
        if max_depth == 0 {
            return Value::Null;
        }
        match source {
            Value::Object(obj) => {
                let mut new_obj = obj.clone();
                for (k, v) in obj {
                    let mut new_keys = key.clone();
                    let var_name = &format!("{k}.");
                    new_keys.push(var_name.as_str());
                    new_obj[k] = resolve_self_variables(v, &new_keys, max_depth - 1);
                }
                Value::Object(new_obj)
            }
            Value::Array(a) => {
                let mut new_arr = Vec::new();
                for v in a {
                    new_arr.push(resolve_self_variables(v, key, max_depth - 1));
                }
                Value::Array(new_arr)
            }
            Value::String(s) if s.contains("$self") => {
                let self_key = key.get(0..key.len() - 1).unwrap_or(&[""]).join("");
                let s = s.replace("$self.", &self_key);
                Value::String(s)
            }
            _ => source.clone(),
        }
    }

    pub fn resolve_variables(
        resolving: &Value,
        source: &Value,
        operations: &Vec<Vec<Operation>>,
        max_depth: usize,
    ) -> Value {
        if max_depth == 0 {
            return Value::Null;
        }
        let mut resolved: Value = json!({});
        match resolving {
            Value::Object(obj) => {
                for (key, value) in obj {
                    resolved[key] = resolve_variables(value, source, operations, max_depth - 1);
                }
            }

            Value::Array(arr) => {
                let mut res_arr = Vec::with_capacity(arr.len());
                for value in arr {
                    res_arr.push(resolve_variables(value, source, operations, max_depth - 1));
                }
                resolved = Value::Array(res_arr);
            }

            Value::String(str) if let Ok(parsed) = str.parse::<ParsedValue>() => match parsed {
                ParsedValue::Variables(ref var)
                    if let Ok(parsed_var) = var.first().unwrap().parse::<ParsedVariable>() =>
                {
                    let path = parsed_var.name.replace('.', "/").parse::<JSPath>().unwrap();

                    let value = path.traverse(source);

                    if let Ok(v) = value {
                        let mut new_ops = operations.clone();
                        new_ops.push(parsed_var.operations);
                        resolved = resolve_variables(v, source, &new_ops, max_depth - 1);
                    } else {
                        resolved = Value::Null;
                    }
                }
                ParsedValue::Color(ref c) => {
                    let mut c = c.clone();
                    let _ = c.update_ops(operations.as_slice());
                    resolved = Value::String(c.to_string());
                }
                ParsedValue::Null => unreachable!(),
                v => resolved = v.into_value(),
            },

            value => {
                resolved = value.clone();
            }
        }

        resolved
    }

    fn handle_value(parsed: ParsedValue, variables: &Value) -> Value {
        match parsed {
            ParsedValue::Variables(ref variable)
                if let Ok(parsed_var) = variable.first().unwrap().parse::<ParsedVariable>()
                    && let Ok(path) = parsed_var.name.replace('.', "/").parse::<JSPath>() =>
            {
                let ops = parsed_var.operations;
                let value = path.traverse(variables).unwrap_or(&Value::Null).clone();
                match value {
                    Value::String(ref v)
                        if !ops.is_empty()
                            && let Ok(parsed) = v.parse::<ParsedValue>()
                            && let ParsedValue::Color(mut color) = parsed =>
                    {
                        let _ = color.update(ops);
                        Value::String(color.to_string())
                    }
                    Value::Null if variable.len() > 1 => {
                        handle_value(ParsedValue::Variables(variable[1..].to_vec()), variables)
                    }
                    _ => value,
                }
            }
            v => v.into_value(),
        }
    }

    pub fn match_variables(template: &Value, variables: &Value) -> Value {
        let mut new_data = template.clone();

        for (key, value) in template.as_object().unwrap() {
            match value {
                Value::String(str) if let Ok(parsed) = str.parse::<ParsedValue>() => {
                    new_data[key] = handle_value(parsed, variables);
                }
                serde_json::Value::Array(a) => {
                    let mut new_arr = Vec::with_capacity(a.len());
                    for value in a {
                        if let Value::Object(_) = value {
                            new_arr.push(match_variables(value, variables));
                        } else if let Value::String(v) = value
                            && let Ok(parsed) = v.parse::<ParsedValue>()
                        {
                            new_arr.push(handle_value(parsed, variables));
                        } else {
                            new_arr.push(value.clone());
                        }
                    }
                    new_data[key] = Value::Array(new_arr);
                }
                serde_json::Value::Object(_) => {
                    new_data[key] = match_variables(value, variables);
                }

                v => new_data[key] = v.clone(),
            }
        }

        new_data
    }

    pub fn replace_regex(key_map: &Map<String, Value>, data: &mut Value, start_path: &str) {
        match data {
            Value::Object(m) => {
                for (k, val) in m.iter_mut() {
                    replace_regex(key_map, val, &format!("{start_path}/{k}"));
                }
            }
            Value::Array(a) => {
                for (k, val) in a.iter_mut().enumerate() {
                    replace_regex(key_map, val, &format!("{start_path}/{k}"));
                }
            }
            _ => {
                for (key, value) in key_map {
                    let rgx = Regex::new(&format!("^{key}$"));
                    if let Ok(regex) = rgx
                        && (regex.find(start_path).is_some() || regex.is_match(start_path))
                        && let Ok(_) = start_path.parse::<JSPath>()
                    {
                        *data = value.clone();
                    }
                }
            }
        }
    }

    pub fn gen(
        mut template: serde_json::Value,
        variables: &serde_json::Value,
    ) -> Result<serde_json::Value, ProgramError> {
        // Step 2: Resolve recursive variables
        let variables = resolve_self_variables(variables, &vec!["$"], MAX_RECURSION_DEPTH);
        let variables = resolve_variables(&variables, &variables, &vec![], MAX_RECURSION_DEPTH);

        // Step 3: Apply Deletions
        if let Some(del_obj) = variables.get("deletions")
            && let Some(deletions) = del_obj.as_object().unwrap().get("keys")
        {
            let deletions = deletions.as_array().unwrap();
            for key in deletions {
                let path = key.as_str().unwrap().parse::<JSPath>().map_err(|_| {
                    ProgramError::Processing(format!(
                        "Invalid path in deletions: {}",
                        key.as_str().unwrap()
                    ))
                })?;

                path.remove(&mut template).unwrap_or_else(|_| {
                    error!("Warning: {} is not a valid deletion path.", key);
                });
            }
        }

        // Step 4: Match variables with template
        let mut matches = match_variables(&template, &variables);

        // Step 5: Apply Overrides
        if let Some(regex_overrides) = variables.get("overrides-regex") {
            let regex_overrides = regex_overrides.as_object().unwrap();
            replace_regex(regex_overrides, &mut matches, "");
        }

        if let Some(overrides) = variables.get("overrides") {
            let overrides = overrides.as_object().unwrap();
            for (key, value) in overrides {
                let path = key.parse::<JSPath>().map_err(|_| {
                    ProgramError::Processing(format!("Invalid path in overrides: {key}"))
                })?;
                path.pave(&mut matches, value.clone())?;
            }
        }

        Ok(matches)
    }
}

fn write_to_file(
    matches: &serde_json::Value,
    generate_names: bool,
    flags: &Flags,
) -> Result<String, ProgramError> {
    // Step 6: Generate the new theme file
    let json_output = serde_json::to_string_pretty(&matches).unwrap();
    let default_name = "/name".parse::<JSPath>().unwrap().traverse(matches).ok();

    let mut file_name = format!("{}.json", {
        if flags.name == "generated-theme"
            && let Some(default) = default_name
        {
            default.as_str().unwrap()
        } else {
            &flags.name
        }
    });

    let out_dir = flags.output_directory.clone();
    let mut out_file = out_dir;

    out_file.push(&file_name);
    if generate_names {
        let mut new_name = String::new();
        while out_file.exists() {
            write!(new_name, "new-").unwrap();
            let mut a = new_name.clone();
            a.push_str(&file_name);
            out_file.pop();
            out_file.push(&a);
        }
        file_name = new_name;
    }

    let mut file = File::create(out_file)
        .map_err(|e| ProgramError::Processing(format!("Could not create file: {e}")))?;
    file.write_all(json_output.as_bytes())
        .map_err(|e| ProgramError::Processing(format!("Could not write to file: {e}")))?;
    Ok(file_name)
}

pub fn generate(
    template: &ValidatedFile,
    mut variables: Vec<ValidatedFile>,
    flags: &[String],
) -> Result<(), ProgramError> {
    let flags = FlagTypes::parse(flags)?;

    let base: serde_json::Value = serde_json::from_reader(&template.file).map_err(|json_err| {
        ProgramError::Processing(format!("Invalid template json: {json_err}"))
    })?;
    let mut template: serde_json::Value = base.clone();
    let mut make_new_files_per_variable = true;
    let mut is_array = false;
    let mut data: serde_json::Value = serde_json::Value::Null;
    let mut generated_files = vec![];

    // Step 0: Traverse to the starting path if it exists
    if let Some(ref starting_path) = flags.path {
        template = starting_path
            .traverse(&template)
            .map_err(|_| ProgramError::Processing(String::from("Invalid starting path.")))?
            .clone();

        match template {
            serde_json::Value::Object(_) => {
                data = serde_json::json!({});
            }
            serde_json::Value::Array(a) => {
                data = serde_json::json!([]);
                is_array = true;
                template = a[0].clone();
            }
            _ => {
                return Err(ProgramError::Processing(String::from(
                    "Starting path must be an object or array.",
                )))
            }
        }

        make_new_files_per_variable = false;
    }

    // Generate Per Each Variable.toml File
    for (i, variable) in variables.iter_mut().enumerate() {
        // Step 1: Deserialize the template and variable files into Objects.
        let vars: serde_json::Value = {
            let mut contents = String::new();
            variable
                .file
                .read_to_string(&mut contents)
                .map_err(|json_err| {
                    ProgramError::Processing(format!("Invalid variable toml: {json_err}"))
                })?;
            serde_json::to_value(
                toml::from_str::<toml::Value>(&contents).map_err(|toml_err| {
                    ProgramError::Processing(format!("Invalid variable toml: {toml_err}"))
                })?,
            )
        }
        .map_err(|json_err| {
            ProgramError::Processing(format!("Invalid variable toml: {json_err}"))
        })?;

        // Step 2-5: Generate the variable matches
        let matches = steps::gen(template.clone(), &vars)?;

        if make_new_files_per_variable {
            // Step 6: Write the new theme file
            generated_files.push(write_to_file(&matches, !flags.replace_name, &flags)?);
        } else if is_array {
            data.as_array_mut().unwrap().push(matches);
        } else {
            data[i] = matches;
        }
    }

    // Step 6: Write the new theme file
    if !make_new_files_per_variable {
        let mut full = base;
        flags.path.clone().unwrap().pave(&mut full, data.clone())?;
        generated_files.push(write_to_file(&full, false, &flags)?);
    }

    println!(
        "Generated ({}) themes: {:?}",
        variables.len(),
        generated_files
    );
    Ok(())
}
