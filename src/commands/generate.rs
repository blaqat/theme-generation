use crate::prelude::*;
use regex::Regex;
use std::{io::Read, path::PathBuf};

/**
Generate:
    Description:
        - Template + Variables = GeneratedTheme
        - This generates a new file by substituting variables in the template file with values from the variable file.
        - This takes the Template as the source of truth. Things in the variable file that arent in the template will be ignored.
        - The generated file will be saved in the current directory.
    Usage:
        substitutor gen template_file variableFile [optional flags]
    Flags:
        -o directory    Set output directory of variable file
        -i directory    Set directory where the .toml files are located
        -p path         Json Path to start the reverse process at
        -n              Name of the output file
        -r              Overwrite the output file of the same name if it exists
*/

const MAX_RECURSION_DEPTH: usize = 8;

#[derive(PartialEq, Debug)]
pub enum GenerateFlags {
    OutputDirectory(PathBuf),
    InputDirectory(PathBuf),
    InnerPath(JsonPath),
    Name(String),
    ReplaceName,
}

#[derive(PartialEq, Debug)]
pub struct Flags {
    replace_name: bool,
    output_directory: PathBuf, // Default to current directory
    input_directory: PathBuf,  // Default to current directory
    name: String,
    path: Option<JsonPath>,
}

impl Flags {
    pub fn directory(&self) -> PathBuf {
        self.input_directory.clone()
    }
}

impl GenerateFlags {
    fn into_vec(flags: Vec<String>) -> Result<Vec<Self>, Error> {
        flags.iter().map(|flag| Self::from_str(flag)).collect()
    }

    pub fn parse(flags: Vec<String>) -> Flags {
        let flags = Self::into_vec(flags).unwrap();
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

        Flags {
            output_directory,
            input_directory,
            name,
            path,
            replace_name,
        }
    }
}

impl FromStr for GenerateFlags {
    type Err = Error;

    fn from_str(flag: &str) -> Result<Self, Error> {
        let get_directory = |path: &str| -> Result<PathBuf, Error> {
            let path = path.replace("~", std::env::var("HOME").unwrap().as_str());
            let path = Path::new(&path);
            if !path.exists() {
                return Err(Error::InvalidFlag(
                    "generate".to_owned(),
                    path.to_str().unwrap().to_owned(),
                ));
            }
            Ok(path.to_path_buf())
        };
        match flag {
            "-r" => Ok(Self::ReplaceName),
            flag if flag.starts_with("-n") => {
                let name = flag.split("=").last().unwrap();
                Ok(Self::Name(name.to_owned()))
            }
            flag if flag.starts_with("-p") => {
                let path = flag.split("=").last().unwrap();
                let path = JsonPath::from_str(path)
                    .map_err(|_| Error::InvalidFlag("reverse".to_owned(), flag.to_owned()))?;
                Ok(Self::InnerPath(path))
            }
            flag if flag.starts_with("-i") => {
                let path = flag.split("=").last().unwrap();
                get_directory(path).map(Self::InputDirectory)
            }
            flag if flag.starts_with("-o") => {
                let path = flag.split("=").last().unwrap();
                get_directory(path).map(Self::OutputDirectory)
            }
            _ => Err(Error::InvalidFlag("reverse".to_owned(), flag.to_owned())),
        }
    }
}

mod steps {
    use super::*;
    use serde_json::json;
    type Value = serde_json::Value;

    pub fn resolve_self_variables(source: &Value, key: &Vec<&str>, _max_depth: usize) -> Value {
        if _max_depth == 0 {
            return Value::Null;
        }
        match source {
            Value::Object(obj) => {
                let mut new_obj = obj.clone();
                for (k, v) in obj.iter() {
                    let mut new_keys = key.clone();
                    let var_name = &format!("{}.", k);
                    new_keys.push(var_name.as_str());
                    new_obj[k] = resolve_self_variables(v, &new_keys, _max_depth - 1);
                }
                Value::Object(new_obj)
            }
            Value::Array(a) => {
                let mut new_arr = Vec::new();
                for v in a.iter() {
                    new_arr.push(resolve_self_variables(v, key, _max_depth - 1));
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
        _source: &Value,
        _operations: &Vec<Vec<ColorChange>>,
        _max_depth: usize,
    ) -> Value {
        if _max_depth == 0 {
            return Value::Null;
        }
        let mut resolved: Value = json!({});
        match resolving {
            Value::Object(obj) => {
                for (key, value) in obj.iter() {
                    resolved[key] = resolve_variables(value, _source, _operations, _max_depth - 1);
                }
            }

            Value::Array(arr) => {
                let mut res_arr = Vec::with_capacity(arr.len());
                for value in arr.iter() {
                    res_arr.push(resolve_variables(
                        value,
                        _source,
                        _operations,
                        _max_depth - 1,
                    ));
                }
                resolved = Value::Array(res_arr);
            }

            Value::String(str) if let Ok(parsed) = str.parse::<ParsedValue>() => match parsed {
                ParsedValue::Variables(ref var)
                    if let Ok(parsed_var) = var.first().unwrap().parse::<ParsedVariable>() =>
                {
                    let path = parsed_var
                        .name
                        .replace(".", "/")
                        .parse::<JsonPath>()
                        .unwrap();

                    let value = path.traverse(_source);

                    if let Ok(v) = value {
                        let mut new_ops = _operations.clone();
                        new_ops.push(parsed_var.operations);
                        resolved = resolve_variables(v, _source, &new_ops, _max_depth - 1);
                    } else {
                        resolved = Value::Null;
                    }
                }
                ParsedValue::Color(ref c) => {
                    let mut c = c.clone();
                    let _ = c.update_ops(_operations.as_slice());
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
            ParsedValue::Variables(ref var)
                if let Ok(parsed_var) = var.first().unwrap().parse::<ParsedVariable>()
                    && let Ok(path) = parsed_var.name.replace(".", "/").parse::<JsonPath>() =>
            {
                let ops = parsed_var.operations;
                let val = path.traverse(variables).unwrap_or(&Value::Null).clone();
                match val {
                    Value::String(ref v)
                        if !ops.is_empty()
                            && let Ok(parsed) = v.parse::<ParsedValue>()
                            && let ParsedValue::Color(mut color) = parsed =>
                    {
                        let _ = color.update(ops);
                        Value::String(color.to_string())
                    }
                    Value::Null if var.len() > 1 => {
                        handle_value(ParsedValue::Variables(var[1..].to_vec()), variables)
                    }
                    _ => val,
                }
            }
            v => v.into_value(),
        }
    }

    pub fn match_variables(template: &Value, variables: &Value) -> Value {
        let mut new_data = template.clone();

        for (key, value) in template.as_object().unwrap().iter() {
            match value {
                Value::String(str) if let Ok(parsed) = str.parse::<ParsedValue>() => {
                    new_data[key] = handle_value(parsed, variables)
                }
                serde_json::Value::Array(a) => {
                    let mut new_arr = Vec::with_capacity(a.len());
                    for value in a.iter() {
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

    pub fn replace_regex(key_map: &Map<String, Value>, data: &mut Value, start_path: String) {
        match data {
            Value::Object(m) => {
                for (k, val) in m.iter_mut() {
                    replace_regex(key_map, val, format!("{}/{}", start_path, k));
                }
            }
            Value::Array(a) => {
                for (k, val) in a.iter_mut().enumerate() {
                    replace_regex(key_map, val, format!("{}/{}", start_path, k));
                }
            }
            _ => {
                for (key, value) in key_map.iter() {
                    let rgx = Regex::new(&format!("^{key}$"));
                    if let Ok(regex) = rgx
                        && (regex.find(&start_path).is_some() || regex.is_match(&start_path))
                        && let Ok(_) = start_path.parse::<JsonPath>()
                    {
                        *data = value.clone();
                    }
                }
            }
        }
    }
}

pub fn generate(
    template: ValidatedFile,
    mut variables: Vec<ValidatedFile>,
    flags: Vec<String>,
) -> Result<(), Error> {
    let flags = GenerateFlags::parse(flags);

    let gen = |mut template: serde_json::Value,
               variables: serde_json::Value|
     -> Result<serde_json::Value, Error> {
        // Step 2: Resolve recursive variables
        let variables = steps::resolve_self_variables(&variables, &vec!["$"], MAX_RECURSION_DEPTH);
        let variables =
            steps::resolve_variables(&variables, &variables, &vec![], MAX_RECURSION_DEPTH);

        // Step 3: Apply Deletions
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

                path.remove(&mut template).unwrap_or_else(|_| {
                    error!("Warning: {} is not a valid deletion path.", key);
                });
            }
        }

        // Step 4: Match variables with template
        let mut matches = steps::match_variables(&template, &variables);

        // Step 5: Apply Overrides
        if let Some(regex_overrides) = variables.get("overrides-regex") {
            let regex_overrides = regex_overrides.as_object().unwrap();
            steps::replace_regex(regex_overrides, &mut matches, String::new())
        }

        if let Some(overrides) = variables.get("overrides") {
            let overrides = overrides.as_object().unwrap();
            for (key, value) in overrides.iter() {
                let path = key.parse::<JsonPath>().map_err(|_| {
                    Error::Processing(format!("Invalid path in overrides: {}", key))
                })?;
                path.pave(&mut matches, value.clone())?;
            }
        }

        Ok(matches)
    };

    let write_to_file = |matches: &serde_json::Value, generate_names: bool| -> Result<(), Error> {
        // Step 6: Generate the new theme file
        let json_output = serde_json::to_string_pretty(&matches).unwrap();
        let default_name = "/name".parse::<JsonPath>().unwrap().traverse(matches).ok();

        let file_name = format!("{}.json", {
            if flags.name == "generated-theme"
                && let Some(default) = default_name
            {
                default.as_str().unwrap()
            } else {
                &flags.name
            }
        });

        let out_dir = flags.output_directory.clone();
        let mut out_file = out_dir.clone();

        out_file.push(&file_name);
        if generate_names {
            let mut new_name = String::from("");
            while out_file.exists() {
                write!(new_name, "new-").unwrap();
                let mut a = new_name.clone();
                a.push_str(&file_name);
                out_file.pop();
                out_file.push(&a);
            }
        }

        let mut file = File::create(out_file)
            .map_err(|e| Error::Processing(format!("Could not create file: {}", e)))?;
        file.write_all(json_output.as_bytes())
            .map_err(|e| Error::Processing(format!("Could not write to file: {}", e)))?;
        Ok(())
    };

    let base: serde_json::Value = serde_json::from_reader(&template.file)
        .map_err(|json_err| Error::Processing(format!("Invalid template json: {}", json_err)))?;
    let mut template: serde_json::Value = base.clone();
    let mut make_new_files_per_variable = true;
    let mut is_array = false;
    let mut data: serde_json::Value = serde_json::Value::Null;

    // Step 0: Traverse to the starting path if it exists
    if let Some(ref starting_path) = flags.path {
        template = starting_path
            .traverse(&template)
            .map_err(|_| Error::Processing(String::from("Invalid starting path.")))?
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
                return Err(Error::Processing(String::from(
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
                    Error::Processing(format!("Invalid variable toml: {}", json_err))
                })?;
            serde_json::to_value(
                toml::from_str::<toml::Value>(&contents).map_err(|toml_err| {
                    Error::Processing(format!("Invalid variable toml: {}", toml_err))
                })?,
            )
        }
        .map_err(|json_err| Error::Processing(format!("Invalid variable toml: {}", json_err)))?;

        // Step 2-5: Generate the variable matches
        let matches = gen(template.clone(), vars)?;

        if !make_new_files_per_variable {
            if is_array {
                data.as_array_mut().unwrap().push(matches);
            } else {
                data[i] = matches;
            }
        } else {
            // Step 6: Write the new theme file
            write_to_file(&matches, !flags.replace_name)?;
        }
    }

    // Step 6: Write the new theme file
    if !make_new_files_per_variable {
        let mut full = base.clone();
        flags.path.clone().unwrap().pave(&mut full, data.clone())?;
        write_to_file(&full, false)?;
    }

    println!("Generated {} files", variables.len());
    Ok(())
}
