use crate::prelude::*;
use serde_json::{json, Value};

/**
Check:
    Description:
        - This checks line by line if the original file and the new file are the same.
        - Displays similarity metrics.
        - Will help in debugging issues in generation/reverse process.
            - Template + Variables = GeneratedTheme == OriginalTheme
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
    fn merge(mut self, other: DiffInfo) -> Self {
        self.diffs.extend(other.diffs);
        self.diffs.sort();
        self.diffs.dedup();
        DiffInfo {
            diffs: self.diffs,
            total_keys: self.total_keys,
        }
    }
}

pub fn json_deep_diff(data1: &Value, data2: &Value, prefix: String, start_keys: usize) -> DiffInfo {
    let local_dne = json!(DNE);
    let mut keys = Vec::new();
    let mut total = start_keys;

    match (data1, data2) {
        (Value::Object(map1), Value::Object(map2)) => {
            for (key, val) in map1.iter() {
                let val2 = map2.get(key).unwrap_or(&local_dne);
                let next_diff = json_deep_diff(val, val2, format!("{prefix}/{key}"), 1);
                keys.extend(next_diff.diffs);
                total += next_diff.total_keys;
            }
        }
        (Value::Array(vec1), Value::Array(vec2)) => {
            for (key, val) in vec1.iter().enumerate() {
                let val2 = vec2.get(key).unwrap_or(&local_dne);
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

pub fn check(file1: ValidatedFile, file2: ValidatedFile) -> Result<(), Error> {
    if file1.format != file2.format {
        return Err(Error::InvalidIOFormat(file2.format.clone()));
    }

    // Step 1: Parse the JSON files
    let data1: Value = serde_json::from_reader(&file1.file)
        .map_err(|_| Error::InvalidIOFormat(file1.format.clone()))?;
    let data2: Value = serde_json::from_reader(&file2.file)
        .map_err(|_| Error::InvalidIOFormat(file2.format.clone()))?;

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

    let percentage = 100.0 - ((diff.diffs.len() as f32 / diff.total_keys as f32) * 100.0);

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
