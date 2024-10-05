use crate::prelude::*;

pub mod serde_value {
    use super::{ProgramError, Value};
    use toml::Value as t_Value;

    pub fn value_to_string(val: &Value) -> String {
        match val {
            Value::String(s) => s.to_owned(),
            _ => val.to_string(),
        }
    }

    pub fn into_toml(val: Value) -> Result<t_Value, ProgramError> {
        match val {
            Value::Null => Ok(t_Value::String(crate::commands::TOML_NULL.to_string())),
            Value::Array(a) => {
                let mut arr = Vec::new();
                for v in a {
                    arr.push(into_toml(v)?);
                }
                Ok(t_Value::Array(arr))
            }
            Value::Object(o) => {
                let mut map = toml::map::Map::new();
                for (k, v) in o {
                    map.insert(k.clone(), into_toml(v)?);
                }
                Ok(t_Value::Table(map))
            }
            a => serde_json::from_value(a)
                .map_err(|e| ProgramError::Processing(format!("Unhandeled theme json: {e}"))),
        }
    }

    pub const fn same_type(a: &Value, b: &Value) -> bool {
        matches!(
            (a, b),
            (Value::Null, Value::Null)
                | (Value::Bool(_), Value::Bool(_))
                | (Value::Number(_), Value::Number(_))
                | (Value::String(_), Value::String(_))
                | (Value::Array(_), Value::Array(_))
                | (Value::Object(_), Value::Object(_))
        )
    }

    pub const fn has_keys(a: &Value) -> bool {
        matches!(a, Value::Object(_) | Value::Array(_))
    }

    pub const fn potential_var(a: &str) -> bool {
        matches!(a.as_bytes(), [b'$' | b'@', ..])
    }

    pub fn potential_set(a: &Value, b: &Value) -> bool {
        match (a, b) {
            (Value::String(a), b) | (b, Value::String(a)) => match (potential_var(a), b) {
                (false, Value::String(b)) => potential_var(b),
                _ => true,
            },
            _ => false,
        }
    }
}

#[derive(Debug, PartialEq, Clone, PartialOrd, Eq, Hash)]
enum JsonKey {
    Key(String),
    Index(usize),
}

impl JsonKey {
    fn inner(&self) -> String {
        match self {
            Self::Key(k) => k.clone(),
            Self::Index(i) => i.to_string(),
        }
    }
}

impl Display for JsonKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.inner())
    }
}

/**
`JsonPath` are strings that represent a path to a value in a JSON object.

For example /a/b/c would be the path to the value 3 in the object {"a": {"b": {"c": 3}}}

/a/1 would be the path to the value 2 in the object {"a": [1, 2, 3]}
*/
#[derive(Debug, PartialEq, Clone, Eq, Hash)]
pub struct JSPath(Vec<JsonKey>);

impl Display for JSPath {
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

impl FromStr for JSPath {
    type Err = ProgramError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let path = s
            .split('/')
            .filter(|x| !x.is_empty())
            .map(|x| x.trim().to_string())
            .map(|x| x.parse::<usize>().map_or(JsonKey::Key(x), JsonKey::Index))
            .collect();
        Ok(Self(path))
    }
}

impl JSPath {
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    pub fn has_num_in_path(&self) -> bool {
        let re = regex::Regex::new(r"/(\d+)(/|$)").unwrap();
        re.captures(self.to_string().as_str())
            .and_then(|cap| cap[1].parse::<i32>().ok())
            .is_some()
    }

    pub fn join(&self) -> String {
        self.0
            .iter()
            .map(std::string::ToString::to_string)
            .reduce(|acc, e| acc + "/" + &e)
            .unwrap_or_default()
    }

    pub fn traverse<'a>(&self, json: &'a Value) -> Result<&'a Value, ProgramError> {
        json.pointer(&format!("{self}"))
            .map_or_else(|| ahh!("Invalid path: {}", self.to_string()), Ok)
    }

    pub fn remove(&self, json: &mut Value) -> Result<(), ProgramError> {
        let (last, rest) = self.0.split_last().unwrap();
        let path = Self(rest.to_vec());

        if let Some(value) = json.pointer_mut(&format!("{path}")) {
            match value {
                Value::Array(a) => {
                    if let JsonKey::Index(idx) = last
                        && *idx < a.len()
                    {
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
                        o.remove(k);
                    }
                },
                _ => unreachable!(),
            }
            Ok(())
        } else {
            ahh!("Invalid path: {}", self.to_string())
        }
    }

    pub fn pave(&self, json: &mut Value, val: Value) -> Result<(), ProgramError> {
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

                        let rest_of_path = Self(self.0[i..].to_vec());
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
                        let rest_of_path = Self(self.0[i..].to_vec());
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

impl Default for JSPath {
    fn default() -> Self {
        Self::new()
    }
}
