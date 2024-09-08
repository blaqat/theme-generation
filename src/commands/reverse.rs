use crate::prelude::*;
use commands::check::{json_deep_diff, DiffInfo};
use core::fmt::{self, Display};
use std::{borrow::BorrowMut, iter::once, path::PathBuf, ptr};
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

// JsonPath are strings that represent a path to a value in a JSON object.
// For example /a/b/c would be the path to the value 3 in the object {"a": {"b": {"c": 3}}}
// /a/1 would be the path to the value 2 in the object {"a": [1, 2, 3]}
#[derive(Debug, PartialEq, Clone)]
struct JsonPath(Vec<JsonKey>);

#[derive(Debug, PartialEq, Clone, Hash, PartialOrd)]
enum JsonKey {
    Key(String),
    Index(usize),
}

impl JsonKey {
    fn key(&self) -> String {
        self.to_string()
    }

    fn inner(&self) -> String {
        match self {
            JsonKey::Key(k) => k.clone(),
            JsonKey::Index(i) => i.to_string(),
        }
    }
}

impl Display for JsonKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.inner())
    }
}

impl Display for JsonPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.0
                .iter()
                .map(|x| x.to_string())
                .collect::<Vec<String>>()
                .join("/")
        )
    }
}

impl FromStr for JsonPath {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let path = s
            .split('/')
            .filter(|x| !x.is_empty())
            .map(|x| x.trim().to_string())
            .map(|x| {
                if let Ok(i) = x.parse::<usize>() {
                    JsonKey::Index(i)
                } else {
                    JsonKey::Key(x)
                }
            })
            .collect();
        Ok(JsonPath(path))
    }
}

impl JsonPath {
    fn from_vec(v: Vec<JsonKey>) -> Self {
        JsonPath(v)
    }

    fn traverse<'a>(&self, json: &'a Value) -> Result<&'a Value, Error> {
        if let Some(value) = json.pointer(&format!("/{}", self)) {
            Ok(value)
        } else {
            ahh!("Invalid path: {}", self.to_string())
        }
    }

    fn set(&self, json: &mut Value, val: Value) -> Result<(), Error> {
        if let Some(value) = json.pointer_mut(&format!("/{}", self)) {
            *value = val;
            Ok(())
        } else {
            ahh!("Invalid path: {}", self.to_string())
        }
    }

    // Pave is similar to set except it creates the things that do not exists
    // For instance with a path "a/b/3/2/c" = 3 if none of those exist it will make
    fn pave(&self, json: &mut Value, val: Value) -> Result<(), Error> {
        let mut current_value = json;
        let len = self.0.len();

        for (i, key) in self.0.iter().enumerate() {
            let is_last = len == i + 1;
            match key {
                JsonKey::Index(idx) => match current_value {
                    Value::Array(arr) => {
                        let len = arr.len();
                        if *idx >= len {
                            arr.resize_with(*idx + 1, || {
                                if is_last {
                                    val.clone()
                                } else {
                                    Value::Null
                                }
                            });
                        }
                        current_value = &mut arr[*idx];
                    }
                    _ if is_last => {
                        let mut temp_arr = Vec::with_capacity(*idx);
                        for _ in 0..=*idx {
                            temp_arr.push(Value::Null);
                        }

                        let rest_of_path = JsonPath::from_vec(self.0[i..].to_vec());
                        let mut rest_of_json = Value::Array(temp_arr);
                        rest_of_path.pave(&mut rest_of_json, val.clone()).unwrap();

                        *current_value = rest_of_json;
                        return Ok(());
                    }
                    _ => return ahh!("Invalid path: {}", self.to_string()),
                },
                JsonKey::Key(k) => match current_value {
                    Value::Object(obj) => {
                        let entry = obj.entry(k).or_insert_with(|| {
                            if is_last {
                                val.clone()
                            } else {
                                Value::Object(Map::new())
                            }
                        });
                        current_value = entry;
                    }

                    _ if is_last => {
                        let rest_of_path = JsonPath::from_vec(self.0[i..].to_vec());
                        let mut rest_of_json = Value::Object(Map::new());
                        rest_of_path.pave(&mut rest_of_json, val.clone()).unwrap();

                        *current_value = rest_of_json;
                        return Ok(());
                    }
                    _ => return ahh!("Invalid path: {}", self.to_string()),
                },
            }
        }

        *current_value = val;

        Ok(())
    }
}

mod test {
    use super::*;

    #[test]
    fn test_json_path() {
        let path = JsonPath::from_str("a/b/c").unwrap();
        assert_eq!(
            path.0,
            vec![
                JsonKey::Key("a".to_string()),
                JsonKey::Key("b".to_string()),
                JsonKey::Key("c".to_string())
            ]
        );

        let path = JsonPath::from_str("a/1").unwrap();
        assert_eq!(
            path.0,
            vec![JsonKey::Key("a".to_string()), JsonKey::Index(1)]
        );

        let path = JsonPath::from_str("/a/1/apple").unwrap();
        assert_eq!(
            path.0,
            vec![
                JsonKey::Key("a".to_string()),
                JsonKey::Index(1),
                JsonKey::Key("apple".to_string())
            ]
        );
    }

    #[test]
    fn test_json_path_to_string() {
        let path = JsonPath::from_str("/a/b/c").unwrap();
        assert_eq!(path.to_string(), "a/b/c");

        let path = JsonPath::from_str("/a/1").unwrap();
        assert_eq!(path.to_string(), "a/1");

        let path = JsonPath::from_str("/a/1/").unwrap();
        assert_eq!(path.to_string(), "a/1");

        let path = JsonPath::from_str("/a/1/2").unwrap();
        assert_eq!(path.to_string(), "a/1/2");
    }

    #[test]
    fn test_json_path_traverse() {
        let json = json!({
            "a": {
                "b": {
                    "c": 3
                }
            }
        });

        let path = JsonPath::from_str("/a/b/c").unwrap();
        assert_eq!(path.traverse(&json).unwrap(), &json!(3));

        let path = JsonPath::from_str("/a/b").unwrap();
        assert_eq!(path.traverse(&json).unwrap(), &json!({"c": 3}));

        let path = JsonPath::from_str("/a").unwrap();
        assert_eq!(
            path.traverse(&json).unwrap(),
            &json!({
                "b": {
                    "c": 3
                }
            })
        );

        let path = JsonPath::from_str("/a/b/c/d").unwrap();
        assert_eq!(path.traverse(&json), ahh!("Invalid path: a/b/c/d"));
    }

    #[test]
    fn test_json_set() {
        let mut json = json!({
            "a": {
                "b": {
                    "c": 3
                }
            },
            "greeting": "goodbye"
        });

        let path = JsonPath::from_str("/a/b/c").unwrap();
        path.set(&mut json, json!(2)).unwrap();
        assert_eq!(json, json!({"a": {"b": {"c": 2}}, "greeting": "goodbye"}));

        let path = JsonPath::from_str("/greeting").unwrap();
        path.set(&mut json, json!("hello")).unwrap();
        assert_eq!(json, json!({"a": {"b": {"c": 2}}, "greeting": "hello"}));
    }

    #[test]
    fn test_json_pave() {
        let mut json = json!({
            "a": {
                "b": {
                    "c": 3
                }
            },
            "greeting": "goodbye"
        });

        let path = JsonPath::from_str("/a/b/c").unwrap();
        path.pave(&mut json, json!(2)).unwrap();
        assert_eq!(json, json!({"a": {"b": {"c": 2}}, "greeting": "goodbye"}));

        let path = JsonPath::from_str("/greeting").unwrap();
        path.pave(&mut json, json!("hello")).unwrap();
        assert_eq!(json, json!({"a": {"b": {"c": 2}}, "greeting": "hello"}));

        let path = JsonPath::from_str("/sleep").unwrap();
        path.pave(&mut json, json!(true)).unwrap();
        assert_eq!(
            json,
            json!({"a": {"b": {"c": 2}}, "greeting": "hello", "sleep": true})
        );

        let path = JsonPath::from_str("/a/b/d").unwrap();
        path.pave(&mut json, json!(4)).unwrap();
        assert_eq!(
            json,
            json!({"a": {"b": {"c": 2, "d": 4}}, "greeting": "hello", "sleep": true})
        );

        let path = JsonPath::from_str("/a/b/0").unwrap();
        path.pave(&mut json, json!(5)).unwrap();
        assert_eq!(
            json,
            json!({"a": {"b": [5] }, "greeting": "hello", "sleep": true})
        );

        let path = JsonPath::from_str("/a/b/2/a").unwrap();
        path.pave(&mut json, json!(6)).unwrap();
        assert_eq!(
            json,
            json!({"a": {"b": [5, null, {"a": 6}]}, "greeting": "hello", "sleep": true})
        );
    }
}

#[derive(PartialEq, Debug)]
enum ReverseFlags {
    Verbose,
    Check,
    Threshold(i32),
    OutputDirectory(PathBuf),
}

impl ReverseFlags {
    fn parse(flags: Vec<String>) -> Result<Vec<Self>, Error> {
        flags.iter().map(|flag| Self::from_str(flag)).collect()
    }
}

impl FromStr for ReverseFlags {
    type Err = Error;

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
}

type VarNames = Vec<String>;

#[derive(Debug, Clone, PartialEq)]
enum ParsedValue<'a> {
    Color(Color),
    Variables(VarNames),
    Value(&'a Value),
    String(String),
    Null,
}

impl Display for ParsedValue<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Color(color) => write!(f, "C({})", color),
            Self::Variables(vars) => write!(f, "V{:?}", vars),
            Self::Value(value) => write!(f, "{}", value),
            Self::String(str) => write!(f, "'{}'", str),
            Self::Null => write!(f, "(N/A)"),
        }
    }
}

impl<'a> FromStr for ParsedValue<'a> {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.starts_with("$") || s.starts_with("@") {
            s.chars().nth(0);
            let vars = s
                .split("|")
                .filter_map(|var| match var.trim().chars().next() {
                    Some('$') => Some(var[1..].to_string()),
                    Some('@') => Some(format!("color.{}", &var[1..])),
                    _ => None,
                })
                .collect();
            Ok(Self::Variables(vars))
        } else if let Ok(color) = s.parse() {
            Ok(Self::Color(color))
        } else {
            Ok(Self::String(s.to_string()))
        }
    }
}

impl<'a> ParsedValue<'a> {
    fn from_value(v: &'a Value) -> Result<Self, Error> {
        match v {
            Value::Null => Ok(Self::Null),
            Value::String(str) => str.parse(),
            _ => Ok(Self::Value(v)),
        }
    }
}

#[derive(Debug, Clone)]
struct ParsedVariable<'a> {
    name: String,
    operations: ColorOperations<'a>,
}

impl Display for ParsedVariable<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, r#""{}":{:?}"#, self.name, self.operations)
    }
}

impl FromStr for ParsedVariable<'_> {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.split_once("..") {
            Some((name, operations)) => {
                let mut chars = operations.chars();
                let operations: ColorOperations = match chars
                    .next()
                    .ok_or_else(|| Error::Processing(format!("Resolving Variable: {}", s)))?
                {
                    // name..(component op val, component op val, ...)
                    '(' if operations.ends_with(")") => operations[1..operations.len() - 1]
                        .split(",")
                        .filter_map(|op| op.parse().ok())
                        .collect(),

                    // name..comp op val
                    comp if comp.is_alphabetic() => vec![operations
                        .parse()
                        .map_err(|e| Error::Processing(format!("Resolving Variable: {:?}", e)))?],

                    // name..val (short hand for alpha = val)
                    val if val.is_ascii_digit() => vec![format!("a.{}", operations)
                        .parse()
                        .map_err(|e| Error::Processing(format!("Resolving Variable: {:?}", e)))?],

                    // name..op val (short hand for alpha op val)
                    _ => vec![format!("a{}", operations)
                        .parse()
                        .map_err(|e| Error::Processing(format!("Resolving Variable: {:?}", e)))?],
                };

                Ok(Self {
                    name: name.to_string(),
                    operations,
                })
            }
            None => Ok(Self {
                name: s.to_string(),
                operations: vec![],
            }),
        }
    }
}

#[derive(Debug, Clone)]
struct SourcedVariable<'a> {
    path: String,
    value: ParsedValue<'a>,
    variables: Vec<Either<String, ParsedVariable<'a>>>,
}

#[derive(Debug, Clone)]
struct ResolvedVariable<'a> {
    path: String,
    value: ParsedValue<'a>,
    variable: ParsedVariable<'a>,
}

impl<'a> ResolvedVariable<'a> {
    fn new(path: String, value: ParsedValue<'a>, variable: ParsedVariable<'a>) -> Self {
        Self {
            path,
            value,
            variable,
        }
    }

    fn from_src(src: &'a SourcedVariable) -> Self {
        let resolved = {
            match src.variables.first() {
                Some(Either::Right(var)) => var,
                _ => unreachable!(),
            }
        };

        Self {
            path: src.path.to_owned(),
            value: src.value.to_owned(),
            variable: resolved.to_owned(),
        }
    }
}

impl std::fmt::Display for SourcedVariable<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut output = String::new();

        let var = self.variables.iter().fold(String::new(), |acc, v| match v {
            Either::Left(var) => format!("{}{} ", acc, var),
            Either::Right(var) => format!("{}{} ", acc, var),
        });

        output.push_str(&format!("{} -> [{}] {}", self.path, var, self.value,));
        write!(f, "{}", output)
    }
}

impl<'a> SourcedVariable<'a> {
    fn new(path: String, var: &str, value: &'a Value) -> SourcedVariable<'a> {
        let value = ParsedValue::from_value(value).unwrap();
        let variables = var
            .split("|")
            .filter_map(|var| match var.trim().chars().next() {
                Some('$') => Some(var[1..].to_string()),
                Some('@') => Some(format!("color.{}", &var[1..])),
                _ => None,
            })
            .map(|v| match v.parse::<ParsedVariable>() {
                Ok(var) => Either::Right(var),
                _ => Either::Left(v.to_string()),
            })
            .collect();

        Self {
            path,
            value,
            variables,
        }
    }
    // List of Reversed values if value is a Color and Operations are present
    fn reversed_values(&self) -> Vec<Color> {
        if let ParsedValue::Color(color) = &self.value {
            self.variables
                .iter()
                .filter_map(|v| match v {
                    Either::Right(var) if !var.operations.is_empty() => Some(&var.operations),
                    _ => None,
                })
                .rev()
                .fold(vec![], |mut acc, op| {
                    let mut new_color = color.clone();
                    let inverse = ColorChange::inverse(op);
                    print!("\n\nOP {} {:?}", &new_color, inverse);
                    new_color.update(inverse);
                    p!(" -> {}\n\n", &new_color);
                    acc.push(new_color);
                    acc
                })
        } else {
            vec![]
        }
    }
}

#[derive(Debug)]
struct KeyDiffInfo<'a> {
    missing: Vec<String>,
    collisions: Vec<String>,
    parsed_vars: Vec<SourcedVariable<'a>>,
}

impl std::fmt::Display for KeyDiffInfo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut output = String::new();
        if !self.missing.is_empty() {
            output.push_str("Missing keys:\n");
            for key in &self.missing {
                output.push_str(&format!("  {}\n", key));
            }
        }
        if !self.collisions.is_empty() {
            output.push_str("Collisions:\n");
            for key in &self.collisions {
                output.push_str(&format!("  {}\n", key));
            }
        }
        if !self.parsed_vars.is_empty() {
            output.push_str("Variables:\n");
            for var in &self.parsed_vars {
                output.push_str(&format!("{}\n", var));
            }
        }
        write!(f, "{}", output)
    }
}

impl KeyDiffInfo<'_> {
    fn extend(&mut self, other: Self) {
        self.missing.extend(other.missing);
        self.collisions.extend(other.collisions);
        self.parsed_vars.extend(other.parsed_vars);
    }
}

fn key_diff<'a>(
    data1: &'a Value,
    data2: &'a Value,
    prefix: String,
    log_vars: bool,
) -> KeyDiffInfo<'a> {
    let mut info = KeyDiffInfo {
        missing: Vec::new(),
        collisions: Vec::new(),
        parsed_vars: Vec::new(),
    };

    match (data1, data2) {
        (Value::Object(map1), Value::Object(map2)) => {
            for (key, val) in map1.iter() {
                match map2.get(key) {
                    Some(val2) => {
                        let next_diff = key_diff(val, val2, format!("{prefix}/{key}"), log_vars);
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
                        let next_diff = key_diff(val, val2, format!("{prefix}[{key}]"), log_vars);
                        info.extend(next_diff);
                    }
                    _ => info.missing.push(format!("{prefix}[{key}]")),
                }
            }
        }

        (Value::String(str), val) | (Value::String(str), val) => {
            if log_vars {
                info.parsed_vars
                    .push(SourcedVariable::new(prefix, str, val))
            }
        }

        (val1, val2) if !log_vars && has_keys(val1) != has_keys(val2) => {
            info.collisions.push(prefix);
        }

        (val1, val2) if !log_vars && same_type(val1, val2) && val1 != val2 => {
            info.collisions.push(prefix);
        }

        _ => (),
    }

    info
}

use std::rc::Rc;
fn resolve_variables(var_diff: KeyDiffInfo<'_>) -> Vec<ResolvedVariable> {
    // HashMap to reference count the variables
    let mut map: HashMap<String, (Vec<&SourcedVariable>, i32)> = HashMap::new();
    let mut resolved = Vec::new();
    let mut unresolved = Vec::new();

    // Iterate over the variables and increment the reference count
    for parsed in &var_diff.parsed_vars {
        for var in &parsed.variables {
            match var {
                Either::Left(_) => todo!(),
                Either::Right(var) => {
                    map.entry(var.name.clone())
                        .and_modify(|(refs, counter)| {
                            refs.push(parsed);
                            *counter += 1;
                        })
                        .or_insert((vec![parsed], 1));
                }
            }
        }
    }

    // Iterate over the variables with reference count > 1 and resolve them
    // The rest are resolved by the first parsed variable
    // Resolving a variable means assigning the variable to the most used value.
    // A color value will need to be reversed if it has operations to ensure the correct value is used for matching.
    for (host_var, (refs, count)) in map {
        if count == 1 {
            let src = refs.first().unwrap();
            resolved.push(ResolvedVariable::from_src(src));
        } else {
            let mut src_values = HashMap::new();
            for src in &refs {
                // p!("{:#?}", src);
                match &src.value {
                    ParsedValue::Color(c) => {
                        let operations = src
                            .variables
                            .iter()
                            .filter_map(|var| match var {
                                Either::Left(_) => None,
                                Either::Right(var) if var.name == host_var => Some(&var.operations),
                                _ => None,
                            })
                            .collect::<Vec<_>>();

                        let reverse = ColorChange::inverse_ops(operations);
                        for op in reverse {
                            let mut base = c.clone();
                            // print!("{:#?} -> ", base.hex);
                            base.update(op);
                            // p!("{:#?}", base.hex);
                            src_values
                                .entry(base.hex.to_string())
                                .and_modify(|(count, _)| *count += 1)
                                .or_insert((1, ParsedValue::Color(base)));
                            // src_values.push(ParsedValue::Color(base));
                        }
                    }
                    ParsedValue::Null => {}
                    s => {
                        src_values
                            .entry(src.value.to_string())
                            .and_modify(|(count, _)| *count += 1)
                            .or_insert((1, s.clone()));
                    }
                }
            }

            let max = src_values
                .iter()
                .max_by_key(|(_, (count, _))| *count)
                .unwrap();

            // Now to go through the srcs and resolve them

            let valid_srcs: Vec<&&SourcedVariable> = refs
                .iter()
                .filter(|src| {
                    let value = match &src.value {
                        ParsedValue::Color(c) => ParsedValue::Color(c.clone()),
                        ParsedValue::Null => ParsedValue::Null,
                        s => s.clone(),
                    };

                    value == max.1 .1
                })
                .collect();

            if valid_srcs.is_empty() {
                let src = refs.first().unwrap();
                resolved.push(ResolvedVariable::from_src(src));

                refs.iter().skip(1).for_each(|s| {
                    unresolved.push(ResolvedVariable::from_src(s));
                });
            } else {
                let src = valid_srcs.first().unwrap();
                resolved.push(ResolvedVariable::from_src(src));
            }
        }
    }

    resolved.dedup_by(|a, b| a.path == b.path);

    p!("RESOLVED: {:#?}", resolved);
    p!("UNRESOLVED {:#?}", unresolved);

    todo!()
}

pub fn reverse(
    template: ValidatedFile,
    theme: ValidatedFile,
    flags: Vec<String>,
) -> Result<(), Error> {
    // p!(
    //     "Template:\n{:?}\n\nTheme:\n{:?}\n\nFlags:\n{:?}",
    //     template,
    //     theme,
    //     ReverseFlags::parse(flags)?
    // );
    // Step 1: Deserialize the template and theme files into Objects.
    let theme: Value = serde_json::from_reader(&theme.file)
        .map_err(|json_err| Error::Processing(format!("Invalid theme json: {}", json_err)))?;
    let template: Value = serde_json::from_reader(&template.file)
        .map_err(|json_err| Error::Processing(format!("Invalid template json: {}", json_err)))?;
    // .map_err(|e| Error::Processing(String::from("Invalid template json.")))?;

    // Step 2: Built Data Structures (Deletions, Overrides, Variables, Colors)
    // let mut deletions: HashMap<String, Value> = get_deletions(&theme, &template);
    let mut var_diff = key_diff(&template, &theme, String::from(""), true);
    let mut override_diff = key_diff(&theme, &template, String::from(""), false);

    _ = resolve_variables(var_diff);
    // p!("Var Diff:\n{} \n\nOverrides:\n{}", var_diff, override_diff);

    // deletion_diff.resolve_variables();

    todo!("REVERSE")
}
