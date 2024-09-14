use crate::prelude::*;
use core::fmt::{self, Display};
use itertools::Itertools;
use std::{cell::RefCell, cmp::Ordering, path::PathBuf};
// use json_patch;
use json::*;
use serde_json::{json, Map, Value};
use steps::*;
use variable::*;

const UNRESOLVED_POINTER_CONST: usize = 2497;
const TOML_NULL: &str = "$none";

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

pub mod json {
    use std::ops::Deref;

    use super::*;

    #[derive(Debug, PartialEq, Clone, PartialOrd, Eq, Hash)]
    enum JsonKey {
        Key(String),
        Index(usize),
    }

    impl JsonKey {
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
    pub struct JsonPath(Vec<JsonKey>);

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
        pub fn new() -> Self {
            JsonPath(Vec::new())
        }

        pub fn join(&self) -> String {
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

        pub fn traverse<'a>(&self, json: &'a Value) -> Result<&'a Value, Error> {
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

        pub fn remove(&self, json: &mut Value) -> Result<(), Error> {
            let (last, rest) = self.0.split_last().unwrap();
            let val_at_last = self.traverse(json)?;
            let path = JsonPath::from_vec(rest.to_vec());

            if let Some(value) = json.pointer_mut(&format!("{}", path)) {
                match value {
                    Value::Array(a) => {
                        if let JsonKey::Index(idx) = last {
                            a.remove(*idx);
                        } else {
                            return ahh!("Invalid path: {}", self.to_string());
                        }
                    }
                    Value::Object(o) => match last {
                        JsonKey::Index(idx) => {
                            o.remove(&idx.to_string());
                        }
                        JsonKey::Key(k) => {
                            // d!(&k);
                            o.remove(k);
                            // d!(&o.get(k));
                        }
                        _ => return ahh!("Invalid path: {}", self.to_string()),
                    },
                    _ => unreachable!(),
                }
                Ok(())
            } else {
                ahh!("Invalid path: {}", self.to_string())
            }
        }

        pub fn pave(&self, json: &mut Value, val: Value) -> Result<(), Error> {
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
}

pub mod variable {
    use super::*;
    use json::*;
    type VarNames = Vec<String>;

    #[derive(Debug, Clone, PartialEq, Hash, Eq)]
    pub enum ParsedValue {
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
            } else if let Ok(potential_color) = s.parse::<ParsedVariable>()
                && let Ok(color) =
                    Color::from_change(&potential_color.name, &potential_color.operations)
            {
                Ok(Self::Color(color))
            } else {
                Ok(Self::String(s.to_string()))
            }
        }
    }

    impl ParsedValue {
        pub fn into_value(self) -> Value {
            match self {
                Self::Color(color) => Value::String(color.to_string()),
                Self::Variables(vars) => Value::String(vars.join("|")),
                Self::Value(value) => value,
                Self::String(str) => Value::String(str),
                Self::Null => Value::Null,
            }
        }

        pub fn from_value(v: &Value) -> Result<Self, Error> {
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
                    _ = color.update(iden_ops);
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
                    _ = color.update_ops(&iden_ops);
                    let color_str = color.to_string();
                    ParsedValue::String(color_str)
                }
                v => v.clone(),
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Hash, Eq)]
    pub struct ParsedVariable {
        pub name: String,
        pub operations: ColorOperations,
    }

    impl ParsedVariable {
        pub fn new() -> Self {
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
        type Err = prelude::Error;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.split_once("..") {
                Some((name, operations)) => {
                    let mut chars = operations.chars();
                    let operations: ColorOperations = match chars.next().ok_or_else(|| {
                        Error::Processing(format!("Resolving Next Variable: {}", s))
                    })? {
                        // name..(component op val, component op val, ...)
                        '(' if operations.ends_with(")") => operations[1..operations.len() - 1]
                            .split(",")
                            .filter_map(|op| op.parse().ok())
                            .collect(),

                        // name..comp op val
                        comp if comp.is_alphabetic()
                            && let Ok(parsed) = operations.parse() =>
                        {
                            vec![parsed]
                        }

                        // name..val (short hand for alpha = val)
                        val if val.is_ascii_hexdigit() => {
                            // d!(&val);
                            vec![format!("a.{}", operations).parse().map_err(|e| {
                                Error::Processing(format!("Resolving Hex Variable: {:?}", e))
                            })?]
                        }

                        // name..op val (short hand for alpha op val)
                        _ => vec![format!("a{}", operations).parse().map_err(|e| {
                            Error::Processing(format!("Resolving Alpha Variable: {:?}", e))
                        })?],
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
    pub struct SourcedVariable {
        pub path: String,
        pub value: ParsedValue,
        pub variables: Vec<Either<String, ParsedVariable>>,
    }

    #[derive(Debug, Clone, Eq, PartialEq, Hash)]
    pub struct ResolvedVariable {
        pub path: JsonPath,
        pub value: ParsedValue,
        pub variables: Vec<ParsedVariable>,
        pub resolved_id: Option<usize>,
    }

    impl<'a> ResolvedVariable {
        pub fn new() -> Self {
            Self {
                path: JsonPath::new(),
                value: ParsedValue::Null,
                variables: Vec::new(),
                resolved_id: None,
            }
        }

        pub fn init(name: &str, value: ParsedValue) -> Self {
            let variable = ParsedVariable {
                name: name.to_string(),
                operations: Vec::new(),
            };

            Self {
                value,
                variables: vec![variable],
                path: JsonPath::new(),
                resolved_id: Some(0),
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

        pub fn from_src(src: &'a SourcedVariable) -> Self {
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

        pub fn from_path(path: &str, json: &Value) -> Self {
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

        pub fn could_resolve(&self) -> bool {
            self.resolved_id.is_none() && !self.variables.is_empty()
        }

        fn resolved(&self) -> Option<&ParsedVariable> {
            match self.resolved_id {
                Some(id) => self.variables.get(id),
                None => None,
            }
        }

        pub fn is_pointer(&self) -> bool {
            self.resolved_id.unwrap_or_default() == UNRESOLVED_POINTER_CONST
        }

        pub fn name(&self) -> String {
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

        pub fn next(&mut self) -> Option<&ParsedVariable> {
            let i = self.resolved_id.map(|i| i + 1).unwrap_or(0);

            if i < self.variables.len() {
                self.resolved_id.replace(i);
                Some(&self.variables[i])
            } else {
                self.resolved_id = None;
                None
            }
        }

        pub fn identity(&self) -> ParsedValue {
            let ops = self.variables.iter().map(|v| &v.operations).collect();
            self.value.identity_ops(ops)
        }

        pub fn results_from(&self, identity: &ParsedValue) -> bool {
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
        pub fn new(path: String, var: &str, value: &Value) -> SourcedVariable {
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
                    Either::Left(var) if used.contains_key(var) => {
                        Some(Either::Left(var.to_string()))
                    }
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
    pub struct KeyDiffInfo {
        pub missing: Vec<String>,
        pub collisions: Vec<String>,
        pub parsed_vars: Vec<SourcedVariable>,
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
        pub fn extend(&mut self, other: Self) {
            self.missing.extend(other.missing);
            self.collisions.extend(other.collisions);
            self.parsed_vars.extend(other.parsed_vars);
        }
    }

    #[derive(Debug)]
    pub struct VariableSet {
        variables: RefCell<HashMap<String, ResolvedVariable>>,
    }

    impl<'a> VariableSet {
        pub fn new() -> Self {
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

        pub fn has_variable(&self, name: &str) -> bool {
            self.variables.borrow().contains_key(name)
        }

        pub fn insert(&self, name: &str, var: ResolvedVariable) {
            self.variables.borrow_mut().insert(name.to_string(), var);
        }

        pub fn inc_insert(&self, name: &str, var: ResolvedVariable) {
            if !self.has_variable(name) {
                self.insert(name, var);
            } else {
                // d!(&name);
                let mut vars = self.variables.borrow_mut();
                let existing = vars.get(name).unwrap().clone();

                let mut count = 1;
                while vars.contains_key(&format!("{}{}", name, count)) {
                    count += 1;
                }

                let existing_name = format!("{}{}", name, count);
                // d!(&existing_name);
                vars.insert(existing_name, existing);
                vars.insert(name.to_owned(), var);
            }
        }

        pub fn safe_insert(&self, name: &str, mut var: ResolvedVariable) {
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

        pub fn to_vec(&self) -> Vec<ResolvedVariable> {
            self.variables.borrow().values().cloned().collect()
        }

        pub fn to_map(&self) -> HashMap<String, ResolvedVariable> {
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

        pub fn get_unresolved(&self) -> Vec<ResolvedVariable> {
            self.variables
                .borrow()
                .values()
                .filter(|v| !v.is_resolvable())
                .cloned()
                .collect()
        }

        pub fn sorted(&self) -> Vec<ResolvedVariable> {
            let mut vars: Vec<_> = self.to_vec();

            vars.sort_by(|a, b| match (a.resolved(), b.resolved()) {
                (Some(a), Some(b)) => a.name.cmp(&b.name),
                (Some(a), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                _ => Ordering::Equal,
            });

            vars
        }

        pub fn resolve(&self) {
            let mut vars = self.variables.borrow_mut();

            let mut resolved = vars
                .clone()
                .into_iter()
                .filter(|(_, v)| v.is_resolvable())
                .collect::<HashMap<_, _>>();

            *vars = resolved;
        }

        pub fn path_sorted(&self) -> Vec<ResolvedVariable> {
            let mut vars: Vec<_> = self.to_vec();
            vars.sort_by(|a, b| a.path.to_string().cmp(&b.path.to_string()));
            vars
        }

        fn is_resolvable(&self) -> bool {
            self.variables.borrow().values().all(|v| v.is_resolvable())
        }
    }

    mod test {
        use super::*;
        use variable::ParsedVariable;
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
    }
}

mod steps {
    use super::variable::*;
    use super::*;

    pub fn resolve_variables(
        var_diff: &KeyDiffInfo,
        mut overrides: Set<ResolvedVariable>,
    ) -> (VariableSet, VariableSet) {
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
            let (mut max, mut rest): (Vec<_>, Vec<_>) =
                imv.iter().partition(|v| v.len() == max_len);
            max.dedup();
            rest.dedup();
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
                // STILL A CHANCE!
                let mut first = true;
                let mut inserted = false;

                let mut current = (*u).clone();
                while current.could_resolve() && !inserted {
                    let mut new = (*u).clone();
                    if first {
                        new.next();
                        first = false;
                    }
                    let mut new_new = new.clone();
                    new.next();
                    match new_new.next() {
                        Some(next) if !var_set.has_variable(&next.name) => {
                            let identity = iden_map
                                .iter()
                                .find(|(_, v)| v.contains(&(s.clone(), *u)))
                                .unwrap()
                                .0;
                            unvar_set.insert(&next.name, new);
                            inserted = true;
                        }
                        Some(_) => current = new.clone(),
                        None => {
                            unvar_set.insert(&var_name.join(), new);
                            inserted = true;
                        }
                    }
                }
                if !inserted {
                    unvar_set.insert(&var_name.join(), (*u).clone());
                }
            }
        }

        var_set.resolve();

        (var_set, unvar_set)
    }

    pub fn key_diff(data1: &Value, data2: &Value, prefix: String, log_vars: bool) -> KeyDiffInfo {
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
                            let next_diff =
                                key_diff(val, val2, format!("{prefix}/{key}"), log_vars);
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
                            let next_diff =
                                key_diff(val, val2, format!("{prefix}/{key}"), log_vars);
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

            (Value::String(str), val) | (val, Value::String(str)) => {
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

    fn get_nested_values(j: &Value) -> Vec<Value> {
        match j {
            Value::Object(map) => map.values().flat_map(get_nested_values).collect(),
            Value::Array(vec) => vec.iter().flat_map(get_nested_values).collect(),
            val => vec![val.clone()],
        }
    }

    type ColorMap = HashMap<String, (String, Vec<Color>)>;
    pub fn to_color_map(v: &VariableSet, o: &VariableSet) -> ColorMap {
        let mut color_map: ColorMap = HashMap::new();
        let get_num_matching_names =
            |n: &str, map: &ColorMap| map.values().filter(|(name, _)| name.starts_with(n)).count();

        let mut update_color_map = |col: &Color| {
            let mut name = col.get_name();
            if name == "404" {
                name = format!("color.{}", color_map.keys().len() + 1)
            }

            let mut name = match col.get_name().as_str() {
                "404" => format!("color.{}", color_map.keys().len()),
                s => s.to_owned(),
            };

            name = match get_num_matching_names(&name, &color_map) {
                0 => name,
                n => format!("{}{}", name, n + 1),
            };

            let colors = color_map.entry(col.to_alphaless_hex()).or_default();
            if colors.0.is_empty() {
                colors.0 = name;
            }
            colors.1.push(col.clone());
        };

        v.to_vec()
            .iter()
            .chain(o.to_vec().iter())
            .for_each(|var| match var.value {
                ParsedValue::Color(ref col) => {
                    update_color_map(col);
                }
                ParsedValue::String(ref s) if let Ok(ref col) = s.parse::<Color>() => {
                    update_color_map(col);
                }
                ParsedValue::Value(ref v) => match v {
                    value if has_keys(value) => {
                        let values = get_nested_values(value);
                        for val in values {
                            match val {
                                Value::String(ref s) if let Ok(ref col) = s.parse::<Color>() => {
                                    update_color_map(col);
                                }
                                _ => (),
                            }
                        }
                    }
                    Value::String(s) if let Ok(ref col) = s.parse::<Color>() => {
                        update_color_map(col);
                    }
                    _ => (),
                },
                _ => (),
            });

        color_map
    }

    pub fn replace_color(val: &ParsedValue, color_map: &ColorMap, threshold: usize) -> ParsedValue {
        let get_color = |c: &Color| {
            let hex = c.to_alphaless_hex();
            let (name, v) = color_map.get(&hex).unwrap();
            if v.len() >= threshold {
                if c.has_alpha() {
                    ParsedValue::String(
                        format!("${}..{}", name, c.get_alpha()).replace("$color.", "@"),
                    )
                } else {
                    ParsedValue::String(format!("${}", name).replace("$color.", "@"))
                }
            } else {
                ParsedValue::String(c.to_string())
            }
        };

        match val {
            ParsedValue::Color(ref col) => get_color(col),
            ParsedValue::String(ref s) if let Ok(ref col) = s.parse::<Color>() => get_color(col),
            ParsedValue::Value(ref v) => match v {
                Value::Array(a) => {
                    let mut new_array = Vec::new();
                    for val in a {
                        let replaced =
                            replace_color(&ParsedValue::Value(val.clone()), color_map, threshold);
                        match replaced {
                            ParsedValue::String(s) => new_array.push(Value::String(s)),
                            ParsedValue::Value(v) => new_array.push(v),
                            ParsedValue::Null => new_array.push(Value::Null),
                            ParsedValue::Color(_) => unreachable!(),
                            ParsedValue::Variables(_) => unreachable!(),
                        }
                    }
                    ParsedValue::Value(Value::Array(new_array))
                }
                Value::Object(o) => {
                    let mut new_obj = Map::new();
                    for (key, val) in o.iter() {
                        let replaced =
                            replace_color(&ParsedValue::Value(val.clone()), color_map, threshold);
                        match replaced {
                            ParsedValue::String(s) => {
                                new_obj.insert(key.to_owned(), Value::String(s))
                            }
                            ParsedValue::Value(v) => new_obj.insert(key.to_owned(), v),
                            ParsedValue::Null => new_obj.insert(key.to_owned(), Value::Null),
                            ParsedValue::Color(c) => unreachable!(),
                            ParsedValue::Variables(_) => unreachable!(),
                        };
                    }
                    ParsedValue::Value(Value::Object(new_obj))
                }
                Value::String(s) => {
                    replace_color(&ParsedValue::String(s.to_owned()), color_map, threshold)
                }
                _ => val.clone(),
            },
            _ => val.clone(),
        }
    }

    /// Order:
    /// 1. Top Level Variables
    /// 2. Color Variables
    /// 3. Grouped Variables
    /// 4. Overrides
    /// 5. Deletions
    pub fn generate_toml_string(
        variables: Value,
        overrides: &VariableSet,
        deletions: &Set<JsonPath>,
    ) -> Result<String, Error> {
        macro_rules! t {
            ($var_name:ident=$from:expr) => {
                let $var_name: toml::Value = {
                    match $from {
                        Value::Null => toml::Value::String(String::from(TOML_NULL)),
                        a => serde_json::from_value(a).map_err(|json_err| {
                            Error::Processing(format!("Invalid theme json: {}", json_err))
                        })?,
                    }
                };
            };
        }

        // let grouped_toml: toml::Value = serde_json::from_value(grouped_json.clone())
        //     .map_err(|json_err| Error::Processing(format!("Invalid theme json: {}", json_err)))?;
        t!(grouped_toml = variables);

        let data = grouped_toml.as_table().unwrap();
        let mut doc = String::new();
        macro_rules! w {
            ($($args:expr),+) => {
                prelude::w!(doc, $($args),+)
            };
        }

        w!("# Reverse Generation Tool Version 3.0");
        d!(data);
        for (k, v) in data
            .iter()
            .filter(|(_, v)| !matches!(v, toml::Value::Table(_)))
        {
            w!("{} = {}", k, v);
        }

        // d!(data);

        w!("\n# Theme Colors");
        w!("[color]");
        for (k, v) in data.iter().filter(|(k, _)| *k == "color") {
            for (color, value) in v
                .as_table()
                .unwrap()
                .iter()
                .sorted_by(|(a, _), (b, _)| a.cmp(b))
            {
                w!("{} = {}", color, value);
            }
        }

        for (k, v) in data.iter().filter(|(k, _)| *k != "color") {
            if v.is_table() {
                w!("\n[{}]", k);
                for (k, v) in v.as_table().unwrap().iter() {
                    match v {
                        toml::Value::Array(a) => {
                            w!("{} = [", k);
                            for (i, v) in a.iter().enumerate() {
                                if i == a.len() - 1 {
                                    w!("\t{}", v);
                                } else {
                                    w!("\t{},", v);
                                }
                            }
                            w!("]");
                        }
                        _ => w!("{} = {}", k, v),
                    }
                }
            }
        }

        w!("\n# Overrides");
        w!("[overrides]");
        // d!(&overrides);
        for (_, v) in overrides
            .to_map()
            .into_iter()
            .sorted_by_key(|(k, _)| k.clone())
        {
            // d!(&k, &v);
            t!(val = v.value.into_value());
            w!(r#""{}" = {}"#, v.path.join(), val);
        }

        w!("\n# Deletions");
        w!("[deletions]");
        w!("keys = [");
        for (i, d) in deletions
            .iter()
            .sorted_by(|(a), (b)| a.to_string().cmp(&b.to_string()))
            .enumerate()
        {
            if i == deletions.len() - 1 {
                w!("\t\"{}\"", d);
            } else {
                w!("\t\"{}\",", d);
            }
        }
        w!("]");

        Ok(doc)
    }
}

#[derive(PartialEq, Debug)]
enum ReverseFlags {
    Verbose,
    Check,
    Threshold(usize),
    OutputDirectory(PathBuf),
    Name(String),
    InnerPath(JsonPath),
}

#[derive(PartialEq, Debug)]
struct Flags {
    verbose: bool,
    check: bool,
    threshold: usize,          // Default to 3
    output_directory: PathBuf, // Default to current directory
    name: String,
    path: Option<JsonPath>,
}

impl ReverseFlags {
    fn into_vec(flags: Vec<String>) -> Result<Vec<Self>, Error> {
        flags.iter().map(|flag| Self::from_str(flag)).collect()
    }

    fn parse(flags: Vec<String>) -> Flags {
        let flags = Self::into_vec(flags).unwrap();
        let mut verbose = false;
        let mut check = false;
        let mut threshold = 3;
        let mut output_directory = PathBuf::from(".");
        let mut name = String::from("reversed-theme");
        let mut path = None;

        for flag in flags {
            match flag {
                Self::Verbose => verbose = true,
                Self::Check => check = true,
                Self::Threshold(value) => threshold = value,
                Self::OutputDirectory(path) => output_directory = path,
                Self::Name(n) => name = n,
                Self::InnerPath(p) => path = Some(p),
            }
        }

        Flags {
            verbose,
            check,
            threshold,
            output_directory,
            name,
            path,
        }
    }
}

impl FromStr for ReverseFlags {
    type Err = Error;

    fn from_str(flag: &str) -> Result<Self, Error> {
        match flag {
            "-v" => Ok(Self::Verbose),
            "-c" => Ok(Self::Check),
            flag if flag.starts_with("-p") => {
                let path = flag.split("=").last().unwrap();
                let path = JsonPath::from_str(path)
                    .map_err(|_| Error::InvalidFlag("reverse".to_owned(), flag.to_owned()))?;
                Ok(Self::InnerPath(path))
            }
            flag if flag.starts_with("-n") => {
                let name = flag.split("=").last().unwrap();
                Ok(Self::Name(name.to_owned()))
            }
            flag if flag.starts_with("-o") => {
                let path = flag.split("=").last().unwrap();
                let path = path.replace("~", std::env::var("HOME").unwrap().as_str());
                let path = Path::new(&path);
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

// Var paths:
// - background.bars.bottombar = #ECEFF4
// ==
// [background.bars] //Group
// bottombar = #ECEFF4
pub fn reverse(
    template: ValidatedFile,
    theme: ValidatedFile,
    flags: Vec<String>,
) -> Result<(), Error> {
    // p!(
    //     "Template:\n{:?}\n\nTheme:\n{:?}\n\nFlags:\n{:?}",
    //     template,
    //     theme,
    //     ReverseFlags::parse(&flags)?
    // );

    let flags = ReverseFlags::parse(flags);

    // Step 1: Deserialize the template and theme files into Objects.
    let mut theme: Value = serde_json::from_reader(&theme.file)
        .map_err(|json_err| Error::Processing(format!("Invalid theme json: {}", json_err)))?;
    let mut template: Value = serde_json::from_reader(&template.file)
        .map_err(|json_err| Error::Processing(format!("Invalid template json: {}", json_err)))?;
    // .map_err(|e| Error::Processing(String::from("Invalid template json.")))?;

    if let Some(starting_path) = flags.path {
        theme = starting_path
            .traverse(&theme)
            .map_err(|_| Error::Processing(String::from("Invalid starting path.")))?
            .clone();

        template = starting_path
            .traverse(&template)
            .map_err(|_| Error::Processing(String::from("Invalid starting path.")))?
            .clone();

        if !same_type(&theme, &template) {
            return Err(Error::Processing(String::from(
                "Starting path types do not match.",
            )));
        }
    }

    let mut reverse = |theme: Value, template: Value, file_name: String| -> Result<(), Error> {
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

        // Step 3: Resolve Variables and Overrides
        let (variables, overrides) = resolve_variables(&var_diff, overrides);
        drop(var_diff);

        // Step 4: Build Color Redundancy Map & Replace Colors
        let mut color_map = to_color_map(&variables, &overrides);
        // d!(&color_map);

        // Step 5: Replace Colors In variables and overrides limited by threshold
        for (var_name, mut var) in variables.to_map().into_iter() {
            let val = replace_color(&var.value, &color_map, flags.threshold);
            // d!(&var_name, &val);
            var.value = val;
            variables.insert(&var_name, var.clone());
        }

        for (var_name, mut var) in overrides.to_map().into_iter() {
            let val = replace_color(&var.value, &color_map, flags.threshold);
            var.value = val;
            overrides.insert(&var_name, var.clone());
        }

        // Step 6: Add Colors to VariablesSet
        for (value, (color, v)) in color_map.iter() {
            if v.len() < flags.threshold {
                continue;
            }
            let first = v.first().unwrap();
            let var = ResolvedVariable::init(color, ParsedValue::String(value.to_owned()));
            variables.inc_insert(color, var);
        }
        drop(color_map);

        // Step 7: Create Groupings
        // e.g varname "a.b.c" = 1, "a.b.d" = 2 should be [a.b] = {c = 1, d = 2}
        // if group only has one key, then it should be merged with the parent group
        // e.g varname "a.b.c" = 1 should be "a.b.c" = 1
        let mut grouped_json = json!({});
        for (var_name, var) in variables.to_map().into_iter() {
            let split = var_name.rsplit_once('.');
            let path = if let Some((group, key)) = split {
                format!("{}/{}", group, key)
            } else {
                var_name.clone()
            }
            .parse::<JsonPath>()
            .unwrap();

            if let ParsedValue::Null = var.value {
                continue;
            }

            path.pave(&mut grouped_json, var.value.into_value());
        }

        // Step 8: Build the Toml Output
        let toml_output = generate_toml_string(grouped_json, &overrides, &deletions)
            .map_err(|e| Error::Processing(format!("Could not generate toml output: {:?}\nThis is probably indicative of needing to use the -p inner path", e)))?;
        let out_dir = flags.output_directory.clone();

        let mut out_file = out_dir.clone();
        let file_name = format!("{}.toml", file_name);
        out_file.push(file_name);

        let mut file = File::create(out_file)
            .map_err(|e| Error::Processing(format!("Could not create file: {}", e)))?;
        file.write_all(toml_output.as_bytes());

        // p!("{}", &doc.to_string());

        // p!("Variables:\n{}", display_vars(&variables, false));
        // p!("Overrides:\n{}", display_vars(&overrides, true));
        // p!("Deletions:\n{}", display_path(&deletions));
        // deletion_diff.resolve_variables();
        Ok(())
    };

    match (&theme, &template) {
        (Value::Object(t), Value::Object(te)) => {
            reverse(theme, template, flags.name)?;
        }
        (Value::Array(theme), Value::Array(template)) => {
            let template = template.first().unwrap();
            for (i, theme) in theme.iter().enumerate() {
                if !same_type(theme, template) {
                    return Err(Error::Processing(format!(
                        "Array index {} types do not match.",
                        i
                    )));
                }
                let default_name = "/name".parse::<JsonPath>().unwrap().traverse(theme).ok();
                let name = {
                    if let Some(name) = default_name {
                        name.as_str().unwrap().to_string()
                    } else {
                        format!("{}{}", flags.name, i)
                    }
                };
                reverse(theme.clone(), template.clone(), name)?;
            }
        }
        _ => return Err(Error::Processing(String::from("Invalid starting path."))),
    }

    Ok(())
}
