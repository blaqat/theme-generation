use serde_json::Value;

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
