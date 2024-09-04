use crate::error;
use crate::prelude::*;
use crate::Error;
use crate::ValidatedFile;

use serde_json::{Map, Value};

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

    let deep_keys_len = deep_num_keys(&data1);
    let data1: HashMap<String, Value> = serde_json::from_value(data1).unwrap();
    let data2: HashMap<String, Value> = serde_json::from_value(data2).unwrap();

    let deep_diff = deep_diff(&data1, &data2, String::new());
    let percentage = (deep_diff.len() as f64 / deep_keys_len as f64) * 100.0;

    p!(
        "{} and {} are {:.1}% different.\nDifferent Keys ({}):\n{:?}",
        &file1.name,
        &file2.name,
        percentage,
        deep_diff.len(),
        deep_diff
    );

    Ok(())
}
