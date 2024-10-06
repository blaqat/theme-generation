use crate::prelude::*;
use std::cell::RefCell;

type VarNames = Vec<String>;
const UNRESOLVED_POINTER_CONST: usize = 2497;

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
            Self::Color(color) => write!(f, "{color}"),
            Self::Variables(vars) => write!(f, "V{vars:?}"),
            Self::Value(value) => write!(f, "{value}"),
            Self::String(str) => write!(f, "'{str}'"),
            Self::Null => write!(f, "NULL"),
        }
    }
}

impl FromStr for ParsedValue {
    type Err = ProgramError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // if s.starts_with('$') || s.starts_with('@') {
        if potential_var(s) {
            s.chars().nth(0);
            let vars = s
                .split('|')
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

    pub fn from_value(v: &Value) -> Result<Self, ProgramError> {
        match v {
            Value::Null => Ok(Self::Null),
            Value::String(str) => str.parse(),
            _ => Ok(Self::Value(v.clone())),
        }
    }

    fn identity_ops(&self, ops: &[&Operations]) -> Self {
        let iden_ops = Operation::identity_ops(ops);
        match self {
            Self::Color(c) => {
                let mut color = c.clone();
                _ = color.update_ops(&iden_ops);
                let color_str = color.to_string();
                Self::String(color_str)
            }
            v => v.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct ParsedVariable {
    pub name: String,
    pub operations: Operations,
}

impl Display for ParsedVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, r#""{}":{:?}"#, self.name, self.operations)
    }
}

impl FromStr for ParsedVariable {
    type Err = prelude::ProgramError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // split can eithe rbe .. or ::, so we need to check for both
        match s.split_once("..").or_else(|| s.split_once("::")) {
            Some((name, operations)) => {
                let mut chars = operations.chars();
                let operations: Operations = match chars.next().ok_or_else(|| {
                    ProgramError::Processing(format!("Resolving Next Variable: {s}"))
                })? {
                    // name..(component op val, component op val, ...)
                    '(' if operations.ends_with(')') => operations[1..operations.len() - 1]
                        .split(',')
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
                        vec![format!("a.{operations}").parse().map_err(|e| {
                            ProgramError::Processing(format!("Resolving Hex Variable: {e:?}"))
                        })?]
                    }

                    // name..op val (short hand for alpha op val)
                    _ => vec![format!("a{operations}").parse().map_err(|e| {
                        ProgramError::Processing(format!("Resolving Alpha Variable: {e:?}"))
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
    pub path: JSPath,
    pub value: ParsedValue,
    pub variables: Vec<ParsedVariable>,
    pub resolved_id: Option<usize>,
    pub siblings: Vec<ResolvedVariable>,
}

impl<'a> ResolvedVariable {
    pub fn init(name: &str, value: ParsedValue) -> Self {
        let variable = ParsedVariable {
            name: name.to_string(),
            operations: Vec::new(),
        };

        Self {
            value,
            variables: vec![variable],
            path: JSPath::new(),
            resolved_id: Some(0),
            siblings: Vec::new(),
        }
    }

    pub fn init_override(path: &str, value: &Value) -> Self {
        let path = JSPath::from_str(path).unwrap();
        let value = ParsedValue::from_value(value).unwrap();
        Self {
            path,
            value,
            variables: Vec::new(),
            resolved_id: Some(0),
            siblings: Vec::new(),
        }
    }

    fn new_pointer(var_name: &str, unresolved_paths: &[String]) -> Self {
        Self {
            path: JSPath::from_str(var_name).unwrap(),
            value: ParsedValue::Variables(unresolved_paths.to_vec()),
            variables: Vec::new(),
            resolved_id: Some(UNRESOLVED_POINTER_CONST),
            siblings: Vec::new(),
        }
    }

    pub fn from_src(src: &'a SourcedVariable) -> Self {
        let variables = src
            .variables
            .iter()
            .filter_map(|v| match v {
                Either::Right(var) => Some(var),
                Either::Left(_) => None,
            })
            .cloned()
            .collect();

        Self {
            path: src.path.parse().unwrap(),
            value: src.value.clone(),
            variables,
            resolved_id: Some(0),
            siblings: Vec::new(),
        }
    }

    pub fn from_path(path: &str, json: &Value) -> Self {
        let path: JSPath = path.parse().unwrap();

        let value = path.traverse(json).map_or(ParsedValue::Null, |val| {
            ParsedValue::from_value(val).unwrap()
        });

        Self {
            path,
            value,
            variables: Vec::new(),
            resolved_id: None,
            siblings: Vec::new(),
        }
    }

    pub fn is_resolvable(&self) -> bool {
        self.resolved_id.map_or(false, |i| i < self.variables.len())
    }

    pub fn could_resolve(&self) -> bool {
        self.resolved_id.is_none() && !self.variables.is_empty()
    }

    fn resolved(&self) -> Option<&ParsedVariable> {
        self.resolved_id.and_then(|id| self.variables.get(id))
    }

    pub fn is_pointer(&self) -> bool {
        self.resolved_id.unwrap_or_default() == UNRESOLVED_POINTER_CONST
    }

    pub fn name(&self) -> String {
        if self.is_resolvable() {
            self.resolved().unwrap().name.clone()
        } else if self.is_pointer() {
            format!("*{}", self.path.join())
        } else {
            self.path.to_string()
        }
    }

    fn unresolve(&mut self) {
        self.resolved_id = None;
    }

    pub fn next(&mut self) -> Option<&ParsedVariable> {
        let i = self.resolved_id.map_or(0, |i| i + 1);

        if i < self.variables.len() {
            self.resolved_id.replace(i);
            Some(&self.variables[i])
        } else {
            self.resolved_id = None;
            None
        }
    }

    pub fn identity(&self) -> ParsedValue {
        let ops: Vec<_> = self.variables.iter().map(|v| &v.operations).collect();
        self.value.identity_ops(&ops)
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

                let _ = b.update_ops(&ops);

                *a == b
            }
            (ParsedValue::Color(_), ParsedValue::String(b))
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
            Either::Left(var) => format!("{acc}{var} "),
            Either::Right(var) => format!("{acc}{var} "),
        });

        output.push_str(&format!("{} -> [{}] {}", self.path, var, self.value,));
        write!(f, "{output}")
    }
}

impl SourcedVariable {
    pub fn new(path: String, var: &str, value: &Value) -> Self {
        let value = ParsedValue::from_value(value).unwrap();
        let variables = var
            .split('|')
            .filter_map(|var| match var.trim().chars().next() {
                Some('$') => Some(var[1..].to_string()),
                Some('@') => Some(format!("color.{}", &var[1..])),
                _ => None,
            })
            .map(|v| {
                v.parse::<ParsedVariable>()
                    .map_or_else(|_| Either::Left(v.to_string()), Either::Right)
            })
            .collect();

        Self {
            path,
            value,
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
                output.push_str(&format!("  {key}\n"));
            }
        }
        if !self.collisions.is_empty() {
            output.push_str("Collisions:\n");
            for key in &self.collisions {
                output.push_str(&format!("  {key}\n"));
            }
        }
        if !self.parsed_vars.is_empty() {
            output.push_str("Variables:\n");
            for var in &self.parsed_vars {
                output.push_str(&format!("{var}\n"));
            }
        }
        write!(f, "{output}")
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

impl VariableSet {
    pub fn new() -> Self {
        Self {
            variables: RefCell::new(HashMap::new()),
        }
    }

    pub fn has_variable(&self, name: &str) -> bool {
        self.variables.borrow().contains_key(name)
    }

    pub fn is_null(&self, name: &str) -> bool {
        self.variables
            .borrow()
            .get(name)
            .and_then(|var| {
                var.variables
                    .iter()
                    .skip(1)
                    .all(|v| self.is_null(&v.name))
                    .then_some(var)
            })
            .map_or(true, |v| v.value == ParsedValue::Null)
    }

    pub fn insert(&self, name: &str, var: ResolvedVariable) {
        self.variables.borrow_mut().insert(name.to_string(), var);
    }

    pub fn insert_sibling(&self, name: &str, var: ResolvedVariable) {
        let mut vars = self.variables.borrow_mut();
        if let Some(og) = vars.get_mut(name) {
            og.siblings.push(var);
        }
    }

    pub fn inc_insert(&self, name: &str, var: ResolvedVariable) {
        if self.has_variable(name) {
            let mut vars = self.variables.borrow_mut();
            let existing = vars.get(name).unwrap().clone();

            let mut count = 1;
            while vars.contains_key(&format!("{name}{count}")) {
                count += 1;
            }

            let existing_name = format!("{name}{count}");
            vars.insert(existing_name, existing);
            vars.insert(name.to_owned(), var);
        } else {
            self.insert(name, var);
        }
    }

    pub fn safe_insert(&self, name: &str, mut var: ResolvedVariable) {
        if !self.has_variable(name) {
            self.insert(name, var);
        } else if var.identity_eq(&self.variables.borrow()[name]) {
            self.insert_sibling(name, var);
        } else {
            let mut vars = self.variables.borrow_mut();
            let mut existing = vars.get(name).unwrap().clone();

            var.unresolve();

            if !existing.is_pointer() {
                existing.unresolve();

                // Insert variables as paths
                let paths: Vec<String> = [var.path.to_string(), existing.path.to_string()]
                    .into_iter()
                    .chain(existing.siblings.iter().map(|s| s.path.to_string()))
                    .collect();

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

    pub fn get_unresolved(&self) -> Vec<ResolvedVariable> {
        self.variables
            .borrow()
            .values()
            .filter(|v| !v.is_resolvable())
            .cloned()
            .collect()
    }

    pub fn resolve(&self) {
        let mut vars = self.variables.borrow_mut();

        let resolved = vars
            .clone()
            .into_iter()
            .filter(|(_, v)| v.is_resolvable())
            .collect::<HashMap<_, _>>();

        *vars = resolved;
    }
}

pub mod special_array {
    use super::*;

    #[derive(Debug)]
    enum MatchMode {
        Exact,
        Contains,
        Regex,
        StartsWith,
        EndsWith,
        NullMismatch,
    }

    impl FromStr for MatchMode {
        type Err = String;
        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "equals" | "match" | "is" | "sameas" | "identical" | "exact" | "=" => {
                    Ok(Self::Exact)
                }
                "includes" | "has" | "within" | "partof" | "contains" | "~" => Ok(Self::Contains),
                "pattern" | "expr" | "dyn" | "regex" | "*" => Ok(Self::Regex),
                "prefix" | "beginswith" | "startswith" | "<" => Ok(Self::StartsWith),
                "suffix" | "trailing" | "endswith" | ">" => Ok(Self::EndsWith),
                "mismatch" | "oneof" | "single" | "xor" | "^" | "!" => Ok(Self::NullMismatch),
                _ => Err("Invalid Match Mode".into()),
            }
        }
    }

    impl MatchMode {
        fn matches(&self, checking: &Value, other_val: &Value) -> bool {
            let check_str = value_to_string(checking);
            match (self, other_val) {
                (Self::Exact, val) => checking == val,
                (Self::Contains | Self::StartsWith | Self::EndsWith | Self::NullMismatch, _)
                    if checking == other_val =>
                {
                    false
                }

                (Self::Contains, Value::String(s)) => s.contains(&check_str),
                (Self::Contains, Value::Array(vec)) => vec.contains(checking),
                (Self::Contains, Value::Object(map)) => map.contains_key(&check_str),

                (Self::Regex, val) => {
                    let re = regex::Regex::new(&check_str).unwrap();
                    // re.is_match(&val.to_string())
                    re.is_match(&value_to_string(val))
                }

                (Self::StartsWith, Value::String(s)) => s.starts_with(&check_str),
                (Self::StartsWith, Value::Array(vec)) => vec.first().is_some_and(|v| checking == v),

                (Self::EndsWith, Value::String(s)) => s.ends_with(&check_str),
                (Self::EndsWith, Value::Array(vec)) => vec.last().is_some_and(|v| checking == v),

                (Self::NullMismatch, Value::Null) => !checking.is_null(),
                (Self::NullMismatch, val) if checking.is_null() => !val.is_null(),

                (Self::StartsWith | Self::EndsWith | Self::Contains | Self::NullMismatch, _) => {
                    false
                }
            }
        }
    }

    #[derive(Debug)]
    enum SpecialMode {
        Single(MatchMode),
        Inside(MatchMode),
    }

    impl FromStr for SpecialMode {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            if let Some((sp_mode, m)) = s.replace('.', "i::").split_once("::") {
                match sp_mode.chars().next().unwrap() {
                    'i' => Ok(Self::Inside(m.parse()?)),
                    's' => Ok(Self::Single(m.parse()?)),
                    _ => Err("Invalid Special Mode".into()),
                }
            } else {
                Ok(Self::Single(s.parse()?))
            }
        }
    }

    impl SpecialMode {
        fn parse_modes(s: &str) -> Result<Vec<Self>, String> {
            s.split('|').map(str::parse).collect()
        }
    }

    #[derive(Debug)]
    pub struct SpecialKey(pub String, Vec<SpecialMode>);

    impl SpecialKey {
        pub fn matches(&self, val1: &Value, other_val: &Value) -> bool {
            self.1
                .iter()
                .map(|mode| match mode {
                    SpecialMode::Single(match_mode) => match_mode.matches(val1, other_val),
                    SpecialMode::Inside(match_mode) => match other_val {
                        Value::Array(vec) => vec.iter().any(|val| match_mode.matches(val1, val)),
                        Value::Object(map) => map.values().any(|val| match_mode.matches(val1, val)),
                        _ => false,
                    },
                })
                .any(|x| x)
        }
    }

    const SPECIAL_ARRAY_KEY: &str = "$::mode";

    pub fn parse_special_keys(vec: &[Value]) -> (bool, bool, Vec<SpecialKey>) {
        let special = vec.first().and_then(|val1| match val1 {
            Value::Object(spobj) if spobj.contains_key(SPECIAL_ARRAY_KEY) => {
                let match_mode = spobj[SPECIAL_ARRAY_KEY].as_str().unwrap_or_default() == "strict";
                let keys = spobj
                    .iter()
                    .filter(|(key, _)| *key != SPECIAL_ARRAY_KEY)
                    .map(|(key, val)| {
                        SpecialKey(
                            key.to_owned(),
                            SpecialMode::parse_modes(val.as_str().unwrap_or_default())
                                .unwrap_or_default(),
                        )
                    })
                    .collect();
                Some((match_mode, keys))
            }

            Value::String(str1) if str1.starts_with("$matches::") => {
                let keys = str1
                    .strip_prefix("$matches::")
                    .unwrap()
                    .split(',')
                    .map(|key| {
                        SpecialKey(key.to_string(), vec![SpecialMode::Single(MatchMode::Exact)])
                    })
                    .collect();

                Some((true, keys))
            }

            _ => None,
        });

        special.map_or_else(Default::default, |val| (true, val.0, val.1))
    }
}
