use core::str;

use crate::prelude::*;
use serde_json::{json, Value};

/**
Check:
    Description:
        - This checks line by line if the original file and the new file are the same.
        - Displays similarity metrics.
        - Will help in debugging issues in generation/reverse process.
            - Template + Variables = `GeneratedTheme` == `OriginalTheme`
    Usage:
        substitutor check originalFile newFile
*/

const DNE: &str = "DNE";

#[derive(Debug)]
pub struct DiffInfo {
    diffs: Vec<String>,
    total_keys: usize,
}

impl DiffInfo {
    fn merge(mut self, other: Self) -> Self {
        self.diffs.extend(other.diffs);
        self.diffs.sort();
        self.diffs.dedup();
        self
    }
}

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
            "equals" | "match" | "is" | "sameas" | "identical" | "exact" | "=" => Ok(Self::Exact),
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

            (Self::StartsWith | Self::EndsWith | Self::Contains | Self::NullMismatch, _) => false,
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

pub fn parse_special_array(vec: &[Value]) -> (bool, bool, Vec<SpecialKey>) {
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
                .map(|key| SpecialKey(key.to_string(), vec![SpecialMode::Single(MatchMode::Exact)]))
                .collect();

            Some((true, keys))
        }

        _ => None,
    });

    special.map_or_else(Default::default, |val| (true, val.0, val.1))
}

pub fn json_deep_diff(data1: &Value, data2: &Value, prefix: String, start_keys: usize) -> DiffInfo {
    let local_dne = json!(DNE);
    let mut keys = Vec::new();
    let mut total = start_keys;

    match (data1, data2) {
        (Value::Object(map1), Value::Object(map2)) => {
            for (key, val) in map1 {
                let val2 = map2.get(key).unwrap_or(&local_dne);
                let next_diff = json_deep_diff(val, val2, format!("{prefix}/{key}"), 1);
                keys.extend(next_diff.diffs);
                total += next_diff.total_keys;
            }
        }
        (Value::Array(vec1), Value::Array(vec2)) => {
            let (is_vec1_spec, match_all, spec_keys_1) = parse_special_array(vec1);
            let (is_vec2_spec, match_all2, spec_keys_2) = parse_special_array(vec2);
            let is_special = is_vec1_spec || is_vec2_spec;
            let match_all = match_all || match_all2;
            let special_keys: Vec<SpecialKey> =
                spec_keys_1.into_iter().chain(spec_keys_2).collect();

            let vec1 = if is_vec1_spec {
                &vec1[1..].to_vec()
            } else {
                vec1
            };

            let vec2 = if is_vec2_spec {
                &vec2[1..].to_vec()
            } else {
                vec2
            };

            /*
            Special Arrays Are arrays made up of objects
            The first element is a string that starts with $:: with keys separated by commas
            These keys are used to find matching objects in the array
            e.g:
            array1 = ["$::matcher", {"matcher": "lalalal", "data": {...}, {"matcher": "frozen_value", "data": {...}}]
            array2 = [{"matcher": "frozen_value", "data": {...}}, ...]

            Should match object with key matcher to the object with key matcher in array2
            */
            for (key, val) in vec1.iter().enumerate() {
                let val2 = if is_special {
                    if !val.is_object() {
                        continue;
                    }
                    let found = vec2
                        .iter()
                        .find(|val2| {
                            val2.is_object()
                                && special_keys
                                    .iter()
                                    .map(|k| {
                                        let val1_key = val.get(&k.0).unwrap_or(&local_dne);
                                        let val2_key = val2.get(&k.0).unwrap_or(&local_dne);
                                        k.matches(val1_key, val2_key)
                                    })
                                    .reduce(|a, b| if match_all { a && b } else { a || b })
                                    .unwrap_or_default()
                        })
                        .unwrap_or(&Value::Null);
                    if found.is_null() {
                        continue;
                    }
                    found
                } else {
                    vec2.get(key).unwrap_or(&local_dne)
                };
                let next_diff = json_deep_diff(val, val2, format!("{prefix}/{key}"), 1);
                keys.extend(next_diff.diffs);
                total += next_diff.total_keys;
            }
        }
        (val1, val2) => {
            let p1 = ParsedValue::from_value(val1);
            let p2 = ParsedValue::from_value(val2);
            if p1 != p2 {
                keys.push(prefix);
            }
        }
    }

    DiffInfo {
        diffs: keys,
        total_keys: total,
    }
}

pub fn check(file1: &ValidatedFile, file2: &ValidatedFile) -> Result<(), ProgramError> {
    if file1.format != file2.format {
        return Err(ProgramError::InvalidIOFormat(file2.format.clone()));
    }

    // Step 1: Parse the JSON files
    let data1: Value = serde_json::from_reader(&file1.file)
        .map_err(|_| ProgramError::InvalidIOFormat(file1.format.clone()))?;
    let data2: Value = serde_json::from_reader(&file2.file)
        .map_err(|_| ProgramError::InvalidIOFormat(file2.format.clone()))?;

    // Step 2: Validate Equivalency
    if data1 == data2 {
        println!(
            "Results for {} and {}: \n---------------------\nSimilarity Percenatage: 100%",
            &file1.name, &file2.name
        );
        return Ok(());
    } else if !data1.is_object() || !data2.is_object() {
        println!(
            "Results for {} and {}: \n---------------------\nSimilarity Percenatage: 0%",
            &file1.name, &file2.name
        );
        return Ok(());
    }

    // Step 3: Deep Diff Calculation
    let diff = {
        let data1: &Value = &data1;
        let data2: &Value = &data2;
        let diff1 = json_deep_diff(data1, data2, String::from("."), 0);
        let diff2 = json_deep_diff(data2, data1, String::from("."), 0);
        diff1.merge(diff2)
    };

    #[allow(clippy::cast_precision_loss)]
    let percentage = (diff.diffs.len() as f32 / diff.total_keys as f32).mul_add(-100.0, 100.0);

    // Step 4: Display Results
    println!(
        "Results for {} and {}: \n---------------------\nSimilarity Percenatage: {:.1}%\nDifferent Keys ({}):\n{:?}",
        &file1.name,
        &file2.name,
        percentage,
        diff.diffs.len(),
        diff.diffs
    );

    Ok(())
}
