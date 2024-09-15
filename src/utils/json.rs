use crate::prelude::*;
use std::ops::Deref;

pub mod serde_value {
    use super::*;

    pub fn same_type(a: &Value, b: &Value) -> bool {
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

    pub fn has_keys(a: &Value) -> bool {
        matches!(a, Value::Object(_) | Value::Array(_))
    }

    pub fn potential_var(a: &str) -> bool {
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
