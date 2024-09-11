use crate::prelude::*;
use commands::check::{json_deep_diff, DiffInfo};
use core::fmt::{self, Display};
use std::{
    borrow::BorrowMut,
    cell::{Ref, RefCell},
    cmp::Ordering,
    iter::once,
    path::PathBuf,
    ptr,
};
use Either::Right;
// use json_patch;
use serde_json::{json, Map, Value};

const UNRESOLVED_POINTER_CONST: usize = 2497;

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

// JsonPath are strings that represent a path to a value in a JSON object.
// For example /a/b/c would be the path to the value 3 in the object {"a": {"b": {"c": 3}}}
// /a/1 would be the path to the value 2 in the object {"a": [1, 2, 3]}
#[derive(Debug, PartialEq, Clone, Eq, Hash)]
struct JsonPath(Vec<JsonKey>);

impl Display for JsonPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.0
                .iter()
                .fold(String::new(), |acc, e| acc + "/" + &e.to_string())
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

    fn join(&self) -> String {
        self.0
            .iter()
            .map(|e| e.to_string())
            .reduce(|acc, e| acc + "/" + &e.to_string())
            .unwrap_or_default()
    }

    fn from_vec(v: Vec<JsonKey>) -> Self {
        JsonPath(v)
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn traverse<'a>(&self, json: &'a Value) -> Result<&'a Value, Error> {
        if let Some(value) = json.pointer(&format!("{}", self)) {
            Ok(value)
        } else {
            ahh!("Invalid path: {}", self.to_string())
        }
    }

    fn set(&self, json: &mut Value, val: Value) -> Result<(), Error> {
        if let Some(value) = json.pointer_mut(&format!("{}", self)) {
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
    fn test_results_from() {
        let mut a = ResolvedVariable::new();
        let mut pa = ParsedVariable::new();
        pa.name = "a".to_string();
        pa.operations = color_change! {
            Red: "+", 30;
        };
        a.variables = vec![pa];
        a.value = ParsedValue::Color(Color::from_hex("#ECEFF4").unwrap());

        let b = ParsedValue::Color(Color::from_hex("#CEEFF4").unwrap());

        assert!(a.results_from(&b));
    }

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
enum ParsedValue {
    Color(Color),
    Variables(VarNames),
    Value(Value),
    String(String),
    Null,
}

impl Display for ParsedValue {
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

impl FromStr for ParsedValue {
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

impl ParsedValue {
    fn from_value(v: &Value) -> Result<Self, Error> {
        match v {
            Value::Null => Ok(Self::Null),
            Value::String(str) => str.parse(),
            _ => Ok(Self::Value(v.clone())),
        }
    }

    fn identity(&self, ops: &ColorOperations) -> Self {
        let iden_ops = ColorChange::identity_op(ops);
        match self {
            ParsedValue::Color(c) => {
                let mut color = c.clone();
                color.update(iden_ops);
                let color_str = color.to_string();
                ParsedValue::String(color_str)
            }
            v => v.clone(),
        }
    }

    fn identity_ops(&self, ops: Vec<&ColorOperations>) -> Self {
        let iden_ops = ColorChange::identity_ops(ops);
        match self {
            ParsedValue::Color(c) => {
                let mut color = c.clone();
                color.update_ops(&iden_ops);
                let color_str = color.to_string();
                ParsedValue::String(color_str)
            }
            v => v.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
struct ParsedVariable {
    name: String,
    operations: ColorOperations,
}

impl ParsedVariable {
    fn new() -> Self {
        Self {
            name: String::new(),
            operations: Vec::new(),
        }
    }
}

impl Display for ParsedVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, r#""{}":{:?}"#, self.name, self.operations)
    }
}

impl FromStr for ParsedVariable {
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
struct SourcedVariable {
    path: String,
    value: ParsedValue,
    variables: Vec<Either<String, ParsedVariable>>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
struct ResolvedVariable {
    path: JsonPath,
    value: ParsedValue,
    variables: Vec<ParsedVariable>,
    resolved_id: Option<usize>,
}

impl<'a> ResolvedVariable {
    fn new() -> Self {
        Self {
            path: JsonPath::new(),
            value: ParsedValue::Null,
            variables: Vec::new(),
            resolved_id: None,
        }
    }

    fn new_pointer(var_name: &str, unresolved_paths: &[String]) -> Self {
        Self {
            path: JsonPath::from_str(var_name).unwrap(),
            value: ParsedValue::Variables(Vec::from_iter(unresolved_paths.iter().cloned())),
            variables: Vec::new(),
            resolved_id: Some(UNRESOLVED_POINTER_CONST),
        }
    }

    fn from_parsed(path: String, value: ParsedValue, variable: ParsedVariable) -> Self {
        Self {
            path: path.parse::<JsonPath>().unwrap(),
            value,
            variables: vec![variable],
            resolved_id: Some(0),
        }
    }

    fn from_src(src: &'a SourcedVariable) -> Self {
        let variables = src
            .variables
            .iter()
            .filter_map(|v| match v {
                Either::Right(var) => Some(var),
                _ => None,
            })
            .cloned()
            .collect();

        Self {
            path: src.path.parse().unwrap(),
            value: src.value.to_owned(),
            variables,
            resolved_id: Some(0),
        }
    }

    fn from_path(path: &str, json: &Value) -> Self {
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
            variables: Vec::new(),
            resolved_id: None,
        }
    }

    fn has_next(&self) -> bool {
        self.resolved_id
            .map_or(!self.variables.is_empty(), |i| i + 1 < self.variables.len())
    }

    fn is_resolvable(&self) -> bool {
        match self.resolved_id {
            Some(i) => i < self.variables.len(),
            None => false,
        }
    }

    fn could_resolve(&self) -> bool {
        self.resolved_id.is_none() && !self.variables.is_empty()
    }

    fn resolved(&self) -> Option<&ParsedVariable> {
        match self.resolved_id {
            Some(id) => self.variables.get(id),
            None => None,
        }
    }

    fn is_pointer(&self) -> bool {
        self.resolved_id.unwrap_or_default() == UNRESOLVED_POINTER_CONST
    }

    fn name(&self) -> String {
        match self.is_resolvable() {
            true => self.resolved().unwrap().name.clone(),
            false if self.is_pointer() => format!("*{}", self.path.join()),
            false => self.path.to_string(),
        }
    }

    fn unresolve(&mut self) {
        self.resolved_id = None;
    }

    fn next_id(&mut self) {
        let i = self.resolved_id.map(|i| i + 1).unwrap_or(0);

        if i < self.variables.len() {
            self.resolved_id.replace(i);
        } else {
            self.resolved_id = None;
        }
    }

    fn next(&mut self) -> Option<&ParsedVariable> {
        let i = self.resolved_id.map(|i| i + 1).unwrap_or(0);

        if i < self.variables.len() {
            self.resolved_id.replace(i);
            Some(&self.variables[i])
        } else {
            self.resolved_id = None;
            None
        }
    }

    fn identity(&self) -> ParsedValue {
        let ops = self.variables.iter().map(|v| &v.operations).collect();
        self.value.identity_ops(ops)
    }

    fn results_from(&self, identity: &ParsedValue) -> bool {
        match (&self.value, identity) {
            (ParsedValue::Color(a), ParsedValue::Color(b)) => {
                let mut b = b.clone();
                let ops: Vec<_> = self
                    .variables
                    .iter()
                    .cloned()
                    .map(|v| v.operations)
                    .collect();

                b.update_ops(&ops);

                // if !a.eq(&b) {
                //     d!(&a, &b);
                // }

                *a == b
            }
            (ParsedValue::Color(a), ParsedValue::String(b))
                if let Ok(color) = b.parse::<Color>() =>
            {
                self.results_from(&ParsedValue::Color(color))
            }
            (a, b) => a == b,
        }
    }

    /// Preferred over `self.value == other.value`
    fn identity_eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}

impl std::fmt::Display for SourcedVariable {
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

impl SourcedVariable {
    fn new(path: String, var: &str, value: &Value) -> SourcedVariable {
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

    /// List of Reversed values if value is a Color and Operations are present
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
struct VariableSet {
    variables: RefCell<HashMap<String, ResolvedVariable>>,
}

impl<'a> VariableSet {
    fn new() -> Self {
        Self {
            variables: RefCell::new(HashMap::new()),
        }
    }

    fn from_slice(vars: &'a [ResolvedVariable]) -> Self {
        Self {
            variables: RefCell::new(
                vars.iter()
                    .map(|v| match v.resolved() {
                        Some(var) => (var.name.to_string(), v.to_owned()),
                        None => (v.path.to_string(), v.to_owned()),
                    })
                    .collect(),
            ),
        }
    }

    fn get(&self, name: &str) -> Option<impl std::ops::Deref<Target = ResolvedVariable> + '_> {
        self.variables
            .borrow()
            .get(name)
            .map(|v| std::cell::Ref::map(self.variables.borrow(), |m| m.get(name).unwrap()))
    }

    fn has_variable(&self, name: &str) -> bool {
        self.variables.borrow().contains_key(name)
    }

    fn insert(&self, name: &str, var: ResolvedVariable) {
        self.variables.borrow_mut().insert(name.to_string(), var);
    }

    fn safe_insert(&self, name: &str, mut var: ResolvedVariable) {
        if !self.has_variable(name) || var.identity_eq(&self.variables.borrow()[name]) {
            self.insert(name, var);
        } else {
            let mut vars = self.variables.borrow_mut();
            let mut existing = vars.get(name).unwrap().clone();

            var.unresolve();

            if !existing.is_pointer() {
                existing.unresolve();

                // Insert variables as paths
                let paths = [var.path.to_string(), existing.path.to_string()];
                vars.insert(paths[0].clone(), var);
                vars.insert(paths[1].clone(), existing);

                // Insert paths as unresolved variable
                let new_var = ResolvedVariable::new_pointer(name, &paths);

                vars.insert(name.to_string(), new_var);
            } else if let ParsedValue::Variables(var_paths) = &mut existing.value {
                var_paths.push(var.path.to_string());
                vars.insert(var.path.to_string(), var);
                vars.insert(name.to_string(), existing);
            }
        }
    }

    fn to_vec(&self) -> Vec<ResolvedVariable> {
        self.variables.borrow().values().cloned().collect()
    }

    fn to_map(&self) -> HashMap<String, ResolvedVariable> {
        self.variables.borrow().clone()
    }

    fn get_set(&self) -> HashMap<String, ResolvedVariable> {
        self.variables.borrow().clone()
    }

    fn get_resolved(&self) -> Vec<ResolvedVariable> {
        self.variables
            .borrow()
            .values()
            .filter(|v| v.is_resolvable())
            .cloned()
            .collect()
    }

    fn get_unresolved(&self) -> Vec<ResolvedVariable> {
        self.variables
            .borrow()
            .values()
            .filter(|v| !v.is_resolvable())
            .cloned()
            .collect()
    }

    fn sorted(&self) -> Vec<ResolvedVariable> {
        let mut vars: Vec<_> = self.to_vec();

        vars.sort_by(|a, b| match (a.resolved(), b.resolved()) {
            (Some(a), Some(b)) => a.name.cmp(&b.name),
            (Some(a), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            _ => Ordering::Equal,
        });

        vars
    }

    fn resolve(&self) {
        let mut vars = self.variables.borrow_mut();

        let mut resolved = vars
            .clone()
            .into_iter()
            .filter(|(_, v)| v.is_resolvable())
            .collect::<HashMap<_, _>>();

        *vars = resolved;
    }

    fn path_sorted(&self) -> Vec<ResolvedVariable> {
        let mut vars: Vec<_> = self.to_vec();
        vars.sort_by(|a, b| a.path.to_string().cmp(&b.path.to_string()));
        vars
    }

    fn is_resolvable(&self) -> bool {
        self.variables.borrow().values().all(|v| v.is_resolvable())
    }
}

fn resolve_variables(
    var_diff: &KeyDiffInfo,
    mut overrides: Set<ResolvedVariable>,
) -> (VariableSet, VariableSet) {
    // HashMap to reference count the variables
    let mut map: HashMap<String, (Vec<&SourcedVariable>, i32)> = HashMap::new();
    let mut resolved = VariableSet::new();
    let mut unresolved = VariableSet::new();
    let mut siblings_map: HashMap<String, _> = HashMap::new();

    macro_rules! INSERT_RESOLVED {
        ($res:expr) => {
            resolved.push($res);
            resolved.sort_by(|a, b| a.variable.name.cmp(&b.variable.name));
        };
    }

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
                    siblings_map
                        .entry(v.name.clone())
                        .or_insert_with(Vec::new)
                        .push((&parsed.path, &parsed.variables));
                }
            }
        }
    }

    let res_var_diff = var_diff
        .parsed_vars
        .iter()
        .filter(|v| !v.variables.is_empty())
        .map(ResolvedVariable::from_src)
        .collect::<Vec<_>>();

    let mut var_set = VariableSet::new();
    let mut unvar_set = VariableSet::new();

    for var in res_var_diff {
        let name = var.name().to_string();
        var_set.safe_insert(&name, var);
    }

    for o in overrides {
        let name = o.name().to_string();
        unvar_set.safe_insert(&name, o);
    }

    let (mut pointers, mut unresolved_vars): (Vec<_>, Vec<_>) = var_set
        .get_unresolved()
        .into_iter()
        .partition(|v| v.is_pointer());

    // d!(&pointers, &unresolved_vars);

    // type UnresolvedSet<'a> = HashMap<JsonPath, Vec<(String, ParsedValue<'a>, ParsedValue<'a>)>>;

    type UnresolvedSet<'a> =
        HashMap<JsonPath, HashMap<ParsedValue, Vec<(String, &'a ResolvedVariable)>>>;

    let mut unresolved_set: UnresolvedSet = HashMap::new();
    for pointer in pointers {
        if let (var_name, ParsedValue::Variables(paths)) = (pointer.path, pointer.value) {
            for path in paths {
                let unresolved_var = unresolved_vars
                    .iter()
                    .find(|v| v.path.to_string() == path)
                    .unwrap();

                let identity = unresolved_var.identity();
                let value = unresolved_var.value.clone();

                unresolved_set
                    .entry(var_name.clone())
                    .or_default()
                    .entry(identity)
                    .or_default()
                    .push((path, unresolved_var));
            }
        }
    }

    // Identities
    for (var_name, iden_map) in &mut unresolved_set {
        let map = iden_map.clone();

        let identities = map.keys().collect::<Vec<_>>();
        let values = map.values().flatten().map(|(_, v)| v).collect::<Vec<_>>();

        identities
            .iter()
            .map(|identity| {
                (
                    identity,
                    values
                        .iter()
                        .filter(|v| v.results_from(identity))
                        .collect::<Vec<_>>(),
                )
            })
            .for_each(|(identity, identity_of)| {
                identity_of.iter().for_each(|v| {
                    iden_map
                        .entry((*identity).clone())
                        .or_default()
                        .push((v.path.to_string(), v));
                });
            });

        let mut imv = iden_map
            .values()
            .map(|v| {
                let mut v = v.clone();
                v.dedup();
                v
            })
            .collect::<Vec<_>>();

        let max_len = imv.iter().map(|v| v.len()).max().unwrap();
        let (mut max, mut rest): (Vec<_>, Vec<_>) = imv.iter().partition(|v| v.len() == max_len);
        max.dedup();
        rest.dedup();
        //     .filter(|v| v.len() == max_len)
        //     .collect::<Vec<_>>();
        // let rest = iden_map
        //     .values()
        //     .filter(|v| v.len() != max_len)
        //     .collect::<Vec<_>>();

        // d!(&var_name, &max_len, &max, &rest);
        let (mut first, max_rest) = (max.first().unwrap(), max.clone());
        let mut first_found = first.first().unwrap();
        let mut first_var = first_found.1.clone();
        let identity = iden_map
            .iter()
            .find(|(_, v)| v.contains(first_found))
            .unwrap()
            .0;
        first_var.value = identity.clone();
        first_var.next();
        var_set.insert(&var_name.join(), first_var);

        rest.extend(max_rest);
        let mut rest = rest
            .into_iter()
            .flatten()
            .filter(|v| !first.contains(v))
            .collect::<Vec<_>>();

        for (s, u) in &rest {
            if u.could_resolve() {
                // STILL A CHANCE!
                let mut new = (*u).clone();
                new.next(); // Current Variable (Value was None)
                let mut new_new = new.clone();
                new.next();
                match new_new.next() {
                    Some(next) => {
                        let identity = iden_map
                            .iter()
                            .find(|(_, v)| v.contains(&(s.clone(), *u)))
                            .unwrap()
                            .0;
                        // d!(&next.name);
                        var_set.insert(&next.name, new);
                    }
                    None => {
                        unvar_set.insert(&var_name.join(), new);
                    }
                }
            } else {
                // NO CHANCE!
                unvar_set.insert(&var_name.join(), (*u).clone());
            }
        }
        // rest.into_iter().filter(|v| !v.is_empty()).for_each(|v| { let mut first = v.first().unwrap().1.clone();
        //     let identity = iden_map
        //         .iter()
        //         .find(|(_, v)| v.contains(v.first().unwrap()))
        //         .unwrap()
        //         .0;
        //     first.value = identity.clone();
        //     first.next();
        //     unvar_set.insert(&var_name.join(), first);
        // });
    }

    var_set.resolve();

    // p!("{}", display_vars(&var_set, false));
    // p!("\n\n{}", display_vars(&unvar_set, false));
    // d!(unresolved_set);

    // Own the variables
    // Move to resolved and unresolved maps

    (var_set, unvar_set)
}

#[derive(Debug)]
struct KeyDiffInfo {
    missing: Vec<String>,
    collisions: Vec<String>,
    parsed_vars: Vec<SourcedVariable>,
}

impl std::fmt::Display for KeyDiffInfo {
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

impl KeyDiffInfo {
    fn extend(&mut self, other: Self) {
        self.missing.extend(other.missing);
        self.collisions.extend(other.collisions);
        self.parsed_vars.extend(other.parsed_vars);
    }
}

fn key_diff(data1: &Value, data2: &Value, prefix: String, log_vars: bool) -> KeyDiffInfo {
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
        (val1, val2) if !log_vars && has_keys(val1) != has_keys(val2) => {
            info.collisions.push(prefix);
        }

        _ => (),
    }

    info
}

fn display_vars(v: &VariableSet, path: bool) -> String {
    let mut out = String::new();

    let v = {
        match path {
            true => v.path_sorted(),
            _ => v.sorted(),
        }
    };

    for res in v {
        let var = match (path) {
            true => format!("{}", res.path),
            // _ if res.is_resolvable() => res.resolved().map(|v| v.name.clone()).unwrap_or_default(),
            _ => res.name(),
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
        out.push_str(&format!("- {}\n", path));
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
