use crate::prelude::*;
use commands::reverse::json::JsonPath;
use palette::convert::IntoColorUnclampedMut;
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
pub enum GenerateFlags {
    Verbose,
    OutputDirectory(PathBuf),
    InputDirectory(PathBuf),
    InnerPath(JsonPath),
    Name(String),
    ReplaceName,
}

#[derive(PartialEq, Debug)]
pub struct Flags {
    verbose: bool,
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
        let mut verbose = false;
        let mut check = false;
        let mut output_directory = PathBuf::from(".");
        let mut input_directory = PathBuf::from(".");
        let mut name = String::from("generated-theme");
        let mut path = None;
        let mut replace_name = false;

        for flag in flags {
            match flag {
                Self::Verbose => verbose = true,
                Self::OutputDirectory(path) => output_directory = path,
                Self::InputDirectory(path) => input_directory = path,
                Self::Name(n) => name = n,
                Self::InnerPath(p) => path = Some(p),
                Self::ReplaceName => replace_name = true,
            }
        }

        Flags {
            verbose,
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
            "-v" => Ok(Self::Verbose),
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
                get_directory(flag.split("=").last().unwrap()).map(Self::InputDirectory)
            }
            flag if flag.starts_with("-o") => {
                let path = flag.split("=").last().unwrap();
                // let path = path.replace("~", std::env::var("HOME").unwrap().as_str());
                // let path = Path::new(&path);
                // if !path.exists() {
                //     return Err(Error::InvalidFlag("reverse".to_owned(), flag.to_owned()));
                // }
                // Ok(Self::OutputDirectory(path.to_path_buf()))
                get_directory(flag.split("=").last().unwrap()).map(Self::OutputDirectory)
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

    pub fn resolve_self_variables(source: &Value, key: &Vec<&str>) -> Value {
        // d!(key);
        match source {
            Value::Object(obj) => {
                // d!(obj);
                let mut new_obj = obj.clone();
                for (k, v) in obj.iter() {
                    let mut new_keys = key.clone();
                    let var_name = &format!("${}.", k);
                    new_keys.push(var_name.as_str());
                    new_obj[k] = resolve_self_variables(v, &new_keys);
                }
                Value::Object(new_obj)
            }
            Value::Array(a) => {
                // d!(a);
                let mut new_arr = Vec::new();
                for v in a.iter() {
                    new_arr.push(resolve_self_variables(v, key));
                }
                Value::Array(new_arr)
            }
            Value::String(s) if s.contains("$self") => {
                let self_key = key.get(key.len() - 2).unwrap_or(&"");
                let mut new_s = s.replace("$self.", self_key);
                // d!(&new_s);
                Value::String(new_s)
            }
            _ => source.clone(),
        }
    }

    pub fn resolve_variables(
        resolving: &Value,
        _source: &Value,
        _operations: &Vec<Vec<ColorChange>>,
    ) -> Value {
        // d!(_source);
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
                match parsed {
                    ParsedValue::Variables(ref var)
                        if let Ok(parsed_var) = var.first().unwrap().parse::<ParsedVariable>() =>
                    {
                        // d!(&parsed_var);
                        let path = parsed_var
                            .name
                            .replace(".", "/")
                            .parse::<JsonPath>()
                            .unwrap();
                        // d!(&path);

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
                        color.update(ops);
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
                    for (i, value) in a.iter().enumerate() {
                        if let Value::Object(v) = value {
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
    mut variables: Vec<ValidatedFile>,
    flags: Vec<String>,
) -> Result<(), Error> {
    // p!(
    //     "Template:\n{:?}\n\nToml:\n{:?}\n\nFlags:\n{:?}",
    //     template,
    //     variables,
    //     GenerateFlags::into_vec(flags)?
    // );
    let flags = GenerateFlags::parse(flags);
    let mut gen = |mut template: serde_json::Value,
                   variables: serde_json::Value|
     -> Result<serde_json::Value, Error> {
        // Step 2: Resolve recursive variables
        let variables = steps::resolve_self_variables(&variables, &vec!["$"]);
        // d!(&variables);
        let variables = steps::resolve_variables(&variables, &variables, &vec![]);

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
                let path = key.parse::<JsonPath>().map_err(|_| {
                    Error::Processing(format!("Invalid path in overrides: {}", key))
                })?;
                path.pave(&mut matches, value.clone());
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
            let mut loop_num = 0;
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
        file.write_all(json_output.as_bytes());
        Ok(())
    };

    let mut base: serde_json::Value = serde_json::from_reader(&template.file)
        .map_err(|json_err| Error::Processing(format!("Invalid template json: {}", json_err)))?;
    let mut template: serde_json::Value = base.clone();
    let mut make_new_files_per_variable = true;
    let mut has_made_file = false;
    let mut is_array = false;
    let mut data: serde_json::Value = serde_json::Value::Null;

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

    for (i, variable) in variables.iter_mut().enumerate() {
        // d!(&variable);
        // Step 1: Deserialize the template and variable files into Objects.
        let vars: serde_json::Value = {
            let mut contents = String::new();
            variable
                .file
                .read_to_string(&mut contents)
                .map_err(|json_err| {
                    Error::Processing(format!("Invalid variable toml: {}", json_err))
                })?;
            serde_json::to_value(toml::from_str::<toml::Value>(&contents).unwrap())
        }
        .map_err(|json_err| Error::Processing(format!("Invalid variable toml: {}", json_err)))?;

        // Generate the new theme file
        let matches = gen(template.clone(), vars)?;

        if !make_new_files_per_variable {
            if is_array {
                data.as_array_mut().unwrap().push(matches);
            } else {
                data[i] = matches;
            }
        } else {
            write_to_file(&matches, !flags.replace_name);
        }
    }

    if !make_new_files_per_variable {
        let mut full = base.clone();
        flags.path.clone().unwrap().pave(&mut full, data.clone());
        write_to_file(&full, false);
    }

    Ok(())
    // todo!()
}
