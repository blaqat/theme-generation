use crate::prelude::*;
use std::{cell::RefCell, cmp::Ordering, path::PathBuf};

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
                let operations: ColorOperations = match chars
                    .next()
                    .ok_or_else(|| Error::Processing(format!("Resolving Next Variable: {}", s)))?
                {
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
    use utils::parsing::ParsedVariable;
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
