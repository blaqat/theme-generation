use crate::prelude::*;
use commands::check::{json_deep_diff, DiffInfo};
use core::fmt::{self, Display};
use std::{borrow::BorrowMut, iter::once, path::PathBuf, ptr};
use Either::Right;
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
#[derive(Debug, PartialEq, Clone, Eq, Hash)]
struct JsonPath(Vec<JsonKey>);

#[derive(Debug, PartialEq, Clone, PartialOrd, Eq, Hash)]
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
    fn new() -> Self {
        JsonPath(vec![])
    }

    fn from_vec(v: Vec<JsonKey>) -> Self {
        JsonPath(v)
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
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

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
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
            Self::Color(color) => write!(f, "{}", color),
            Self::Variables(vars) => write!(f, "V{:?}", vars),
            Self::Value(value) => write!(f, "{}", value),
            Self::String(str) => write!(f, "'{}'", str),
            Self::Null => write!(f, "NULL"),
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

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
struct ParsedVariable<'a> {
    name: String,
    operations: ColorOperations<'a>,
}

impl ParsedVariable<'_> {
    fn new() -> Self {
        Self {
            name: String::new(),
            operations: Vec::new(),
        }
    }
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

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
struct SourcedVariable<'a> {
    path: String,
    value: ParsedValue<'a>,
    variables: Vec<Either<String, ParsedVariable<'a>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ResolvedVariable<'a> {
    path: JsonPath,
    value: ParsedValue<'a>,
    variable: ParsedVariable<'a>,
}

impl<'a> ResolvedVariable<'a> {
    fn new(path: String, value: ParsedValue<'a>, variable: ParsedVariable<'a>) -> Self {
        Self {
            path: path.parse::<JsonPath>().unwrap(),
            value,
            variable,
        }
    }

    fn new_pathless(value: ParsedValue<'a>, variable: &'a SourcedVariable) -> Self {
        let resolved = {
            match variable.variables.first() {
                Some(Either::Right(var)) => var,
                _ => unreachable!(),
            }
        };
        Self {
            path: JsonPath::new(),
            value,
            variable: resolved.to_owned(),
        }
    }

    fn is_pathless(&self) -> bool {
        self.path.is_empty()
    }

    fn from_src(src: &'a SourcedVariable) -> Self {
        let resolved = {
            match src.variables.first() {
                Some(Either::Right(var)) => var,
                _ => unreachable!(),
            }
        };

        Self {
            path: src.path.parse().unwrap(),
            value: src.value.to_owned(),
            variable: resolved.to_owned(),
        }
    }

    fn from_path(path: &str, json: &'a Value) -> Self {
        let path: JsonPath = path.parse().unwrap();

        let value = {
            match path.traverse(json) {
                Ok(val) => ParsedValue::from_value(val).unwrap(),
                _ => ParsedValue::Null,
            }
        };

        Self {
            path,
            value,
            variable: ParsedVariable::new(),
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

    fn filter_used(&self, used: &HashMap<String, bool>) -> Self {
        let variables = self
            .variables
            .iter()
            .filter_map(|v| match v {
                Either::Left(var) if used.contains_key(var) => Some(Either::Left(var.to_string())),
                Either::Right(var) if used.contains_key(&var.name) => {
                    Some(Either::Right(var.to_owned()))
                }
                _ => None,
            })
            .collect();

        Self {
            path: self.path.to_string(),
            value: self.value.to_owned(),
            variables,
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
                        let next_diff = key_diff(val, val2, format!("{prefix}/{key}"), log_vars);
                        info.extend(next_diff);
                    }
                    _ => info.missing.push(format!("{prefix}/{key}")),
                }
            }
        }

        (val1, val2) if !log_vars && has_keys(val1) != has_keys(val2) => {
            info.collisions.push(prefix);
        }

        (val1, val2) if !log_vars && same_type(val1, val2) && val1 != val2 => {
            if !potential_set(val1, val2) {
                info.collisions.push(prefix);
            } else if log_vars && let (Value::String(str), val) = (val1, val2) {
                info.parsed_vars
                    .push(SourcedVariable::new(prefix, str, val))
            }
        }

        (Value::String(str), val) | (Value::String(str), val) => {
            if log_vars {
                info.parsed_vars
                    .push(SourcedVariable::new(prefix, str, val))
            }
        }

        _ => (),
    }

    info
}

fn resolve_variables<'a>(
    var_diff: &'a KeyDiffInfo,
    mut overrides: Set<ResolvedVariable<'a>>,
) -> (Set<ResolvedVariable<'a>>, Set<ResolvedVariable<'a>>) {
    // HashMap to reference count the variables
    let mut map: HashMap<String, (Vec<&SourcedVariable>, i32)> = HashMap::new();
    let mut resolved = Set::new();
    let mut unresolved = Set::new();
    let mut siblings_map: HashMap<String, _> = HashMap::new();

    // Iterate over the variables and increment the reference count
    for parsed in &var_diff.parsed_vars {
        for var in &parsed.variables {
            match var {
                Either::Left(_) => todo!(),
                Either::Right(v) => {
                    map.entry(v.name.clone())
                        .and_modify(|(refs, counter)| {
                            refs.push(parsed);
                            *counter += 1;
                        })
                        .or_insert((vec![parsed], 1));
                    // siblings_map.insert(v.name.clone(), &parsed.variables);
                    siblings_map
                        .entry(v.name.clone())
                        .or_insert_with(Vec::new)
                        .push((&parsed.path, &parsed.variables));
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
            resolved.insert(ResolvedVariable::from_src(src));
        } else {
            let mut identity_values: HashMap<String, Vec<&SourcedVariable>> = HashMap::new();

            // 1. Group variables by IDENTITY Value
            for src in &refs {
                let value = &src.value;
                let ops = src
                    .variables
                    .iter()
                    .filter_map(|var| match var {
                        Either::Left(_) => None,
                        Either::Right(var) => Some(&var.operations),
                    })
                    .collect::<Vec<_>>();

                let iden_ops = ColorChange::identitiy_ops(ops);

                match value {
                    ParsedValue::Color(c) => {
                        let mut color = c.clone();
                        color.update_ops(&iden_ops);
                        let color_str = color.to_string();
                        let vals = identity_values.entry(color_str).or_default();
                        vals.push(src);
                    }
                    _ => {
                        let mut vals = identity_values.entry(value.to_string()).or_default();
                        vals.push(src);
                    }
                }
            }

            // 2. Find the most used identity value
            let (max_key, max) = identity_values
                .iter()
                .max_by(|a, b| a.1.len().cmp(&b.1.len()))
                .unwrap();

            let all_max = identity_values.values().all(|v| v.len() == max.len());

            let match_source = |src: &&SourcedVariable| {
                let ops = src
                    .variables
                    .iter()
                    .filter_map(|var| match var {
                        Either::Left(_) => None,
                        Either::Right(var) => Some(&var.operations),
                    })
                    .cloned()
                    .collect::<Vec<_>>();

                identity_values
                    .iter()
                    .filter(|(iden, _)| match iden.parse::<Color>() {
                        Ok(color) => {
                            let mut color = color.clone();
                            color.update_ops(&ops);
                            color.to_string() == src.value.to_string()
                        }
                        _ => **iden == src.value.to_string(),
                    })
                    .collect::<HashMap<_, _>>()
            };

            if !all_max {
                for (key, src) in &identity_values {
                    if key == max_key {
                        let src = src.first().unwrap();
                        resolved.insert(ResolvedVariable::from_src(src));
                    } else {
                        for src in src {
                            let matches = match_source(src);
                            if !matches.contains_key(max_key) {
                                unresolved.insert(ResolvedVariable::from_src(src));
                            }
                        }
                    }
                }
            } else {
                let mut matches: HashMap<&String, Set<_>> = HashMap::new();
                identity_values.iter().for_each(|(key, v)| {
                    matches.insert(key, Set::from_iter(v.iter()));
                });
                for (key, source) in &identity_values {
                    for src in source {
                        let m = match_source(src);
                        // p!("{} {:?}", src.path, m.keys());
                        m.keys().for_each(|k| {
                            // println!("SRC:\n{:?}\nMATCHES\n{:?}\n\n", (key, &src.path), k);
                            let mut v = matches.entry(k).or_default();
                            v.insert(src);
                        });
                    }
                }

                // d!(&matches);
                let (max_key, max) = matches
                    .iter()
                    .max_by(|a, b| a.1.len().cmp(&b.1.len()))
                    .unwrap();
                let all_max = matches.values().all(|v| v.len() == max.len());
                // d!(&matches, max_key, all_max, max.len());

                if !all_max {
                    let first_src = max.iter().next().unwrap();
                    let var = ResolvedVariable::new_pathless(
                        ParsedValue::from_str(max_key).unwrap(),
                        first_src,
                    );
                    resolved.insert(var);
                    for (key, sources) in &matches {
                        if key != max_key {
                            for src in sources {
                                if !max.contains(src) {
                                    unresolved.insert(ResolvedVariable::from_src(src));
                                }
                            }
                        }
                    }
                } else {
                    let first_src = max.iter().next().unwrap();
                    resolved.insert(ResolvedVariable::from_src(first_src));

                    for (key, src) in &identity_values {
                        for src in src {
                            if !max.contains(src) {
                                unresolved.insert(ResolvedVariable::from_src(src));
                            }
                        }
                    }
                }
            }
        }
    }

    // Resolve multivariables
    // Rules:
    // 1. There can only be one resolved variable "X".
    // 2. If variable "X" = "Y" exists and "X" = "Z" exists,
    //  AND X has no sibling variables, then Override "X"'s path to Y
    //  AND X has sibling variables Z, then X = Z until there is no resolved Z
    //
    // To put in plain terms, If there are two resolved values with the same variable, one must move to another variable or be overriden

    // Group resolved variables by name
    let mut need_fix: HashMap<String, (bool, _)> = HashMap::new();

    for var in &resolved {
        match need_fix.get(&var.variable.name) {
            Some(_) => {
                need_fix.insert(var.variable.name.clone(), (true, var));
            }
            None => {
                need_fix.insert(var.variable.name.clone(), (false, var));
            }
        }
    }

    // d!(&need_fix, &siblings_map);
    let mut sub_resolved = HashMap::new();
    let mut sub_unresolved = HashMap::new();

    for (name, (nf, variable)) in need_fix {
        let path = format!("/{}", variable.path);
        // If nf is true there are multiple variables with the same name.
        //
        // If variable has any siblings not resolved, then take the first non-resolved siblings and replace the variable's name with it
        // If all siblings are resolved, then add the variable to the unresolved set

        // If nf is false, then the variable is resolved and there are no siblings so can ignore
        if !nf {
            continue;
        }

        // Get the siblings of the variable
        if let Some(siblings) = siblings_map.get(&name) {
            let mut found_unresolved = false;

            for sibling in siblings {
                if path != *sibling.0 {
                    // d!(&variable.path.to_string(), sibling.0);
                    continue;
                }
                for sibling_var in sibling.1 {
                    if let Either::Right(sv) = sibling_var {
                        if !resolved.iter().any(|v| v.variable.name == sv.name) {
                            // Found an unresolved sibling, replace the variable's name with it
                            let mut new_variable = variable.clone();
                            new_variable.variable.name = sv.name.clone();
                            sub_resolved.insert(variable.path.clone(), new_variable);
                            found_unresolved = true;
                        }
                    }
                }
            }

            if !found_unresolved {
                // All siblings are resolved, add the variable to the unresolved set
                sub_unresolved.insert(variable.path.clone(), variable.clone());
            }
        }
    }

    d!(&sub_resolved, &sub_unresolved);

    d!(&unresolved);
    // d!(&resolved, &sub_resolved);

    for (path, variable) in sub_resolved {
        unresolved.retain(|v| v.path != path);
        resolved.retain(|v| v.path != path);
        resolved.insert(variable);
    }

    for (path, variable) in sub_unresolved {
        resolved.retain(|v| v.path != path);
        unresolved.retain(|v| v.path != path);
        unresolved.insert(variable);
    }

    // d!(&resolved, &overrides);

    overrides.extend(unresolved);

    for var in &resolved {
        overrides.retain(|v| v.path != var.path);
    }

    (resolved, overrides)
}

fn display_vars(v: &Set<ResolvedVariable>, path: bool) -> String {
    let mut out = String::new();
    let mut v = v.iter().collect::<Vec<_>>();
    v.sort_by(|a, b| match (path) {
        true => a.path.to_string().cmp(&b.path.to_string()),
        _ => a.variable.name.cmp(&b.variable.name),
    });
    for res in v {
        let var = match (path) {
            true => format!("/{}", res.path),
            _ => res.variable.name.clone(),
        };
        let val = res.value.to_string();
        out.push_str(&format!("- {} = {}\n", var, val));
    }
    out
}

fn display_path(v: &Set<JsonPath>) -> String {
    // format!("- {}", v.to_string())
    let mut out = String::new();
    let mut v = v.iter().collect::<Vec<_>>();
    v.sort_by_key(|p| p.to_string());
    for path in v {
        out.push_str(&format!("- /{}\n", path));
    }
    out
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
    let var_diff = key_diff(&template, &theme, String::from(""), true);
    let override_diff = key_diff(&theme, &template, String::from(""), false);
    // d!(&var_diff, &override_diff);

    let mut overrides: Set<_> = override_diff
        .missing
        .iter()
        .chain(override_diff.collisions.iter())
        .map(|key| ResolvedVariable::from_path(key, &theme))
        .collect();

    let mut deletions: Set<_> = var_diff
        .missing
        .iter()
        .map(|key| key.parse::<JsonPath>().unwrap())
        .collect();

    let (variables, overrides) = resolve_variables(&var_diff, overrides);

    p!("Variables:\n{}", display_vars(&variables, false));
    p!("Overrides:\n{}", display_vars(&overrides, true));
    p!("Deletions:\n{}", display_path(&deletions));

    // deletion_diff.resolve_variables();

    todo!("REVERSE")
}
