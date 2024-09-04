use crate::error;
use crate::prelude::*;
use crate::Error;
use crate::ValidatedFile;
use json_patch;

use serde_json::{json, Map, Value};

trait HasDepth {
    fn has_depth(&self) -> bool;
}

impl HasDepth for Value {
    fn has_depth(&self) -> bool {
        self.is_object() || self.is_array()
    }
}

const DNE: &'static str = "DNE";

#[derive(Debug)]
struct DiffInfo {
    diffs: Vec<String>,
    total_keys: usize,
}

impl DiffInfo {
    fn merge(&mut self, other: &mut DiffInfo) -> Self {
        let mut new_diffs = self.diffs.clone();
        new_diffs.append(&mut other.diffs);
        new_diffs.dedup();
        DiffInfo {
            diffs: new_diffs,
            total_keys: self.total_keys,
        }
    }
}

fn json_two_diff(data1: &Value, data2: &Value) -> DiffInfo {
    json_deep_diff(data1, data2, String::from(""), 0).merge(&mut json_deep_diff(
        data2,
        data1,
        String::from(""),
        0,
    ))
}

fn json_deep_diff(data1: &Value, data2: &Value, prefix: String, start_keys: usize) -> DiffInfo {
    let dne = json!(DNE);
    let mut keys = vec![];
    let mut total = start_keys;

    match (data1, data2) {
        (Value::Object(map1), Value::Object(map2)) => {
            for (key, val) in map1.iter() {
                let val2 = map2.get(key).unwrap_or(&dne);
                let mut next_diff = json_deep_diff(val, val2, format!("{prefix}/{key}"), 1);
                keys.append(&mut next_diff.diffs);
                total += next_diff.total_keys;
            }
        }
        (Value::Array(vec1), Value::Array(vec2)) => {
            for (key, val) in vec1.iter().enumerate() {
                let val2 = vec2.get(key).unwrap_or(&dne);

                let mut next_diff = json_deep_diff(val, val2, format!("{prefix}/{key}"), 1);
                keys.append(&mut next_diff.diffs);
                total += next_diff.total_keys;
            }
        }
        (val1, val2) => {
            let str1 = val1.to_string();
            let str2 = val2.to_string();
            if str1.to_lowercase() != str2.to_lowercase() {
                keys.push(prefix);
            }
        }
    }

    DiffInfo {
        diffs: keys,
        total_keys: total,
    }
}

pub fn check(file1: ValidatedFile, file2: ValidatedFile) -> Result<(), Error> {
    if file1.format != file2.format {
        return Err(Error::InvalidIOFormat(file2.format.to_string()));
    }

    let data1: Value =
        serde_json::from_reader(&file1.file).expect("Expected a properly formatted JSON file");
    let data2: Value =
        serde_json::from_reader(&file2.file).expect("Expected a properly formatted JSON file");

    if data1 == data2 {
        return Ok(p!("{} and {} are 100% the same.", &file1.name, &file2.name));
    } else if !data1.is_object() || !data2.is_object() {
        p!("{} and {} are different types.", &file1.name, &file2.name)
    }

    // let diff = json_patch::diff(&data1, &data2);
    let diff = json_deep_diff(&data1, &data2, String::from(""), 0);
    p!("{:?}", diff);

    // let diff2 = json_deep_diff(&data2, &data1, String::from(""), 0);
    // p!("{:?}", diff2);

    // todo!();

    let percentage = (diff.diffs.len() as f32 / diff.total_keys as f32) * 100.0;

    p!(
        "{} and {} are {:.1}% different.\nDifferent Keys ({}):\n{:?}",
        &file1.name,
        &file2.name,
        percentage,
        diff.diffs.len(),
        diff.diffs
    );

    Ok(())
}
