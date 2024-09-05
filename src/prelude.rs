//! Crate preulde
pub use crate::*;
pub use anyhow;
pub use std::{
    collections::HashMap,
    env::args,
    fs::{read_dir, File},
    println as p,
};
pub use utils::*;

// Red error macro
#[macro_export]
macro_rules! error {
    ( $($args:expr),+ ) => {
        eprintln!("\x1b[31m{}\x1b[0m", format!($($args),+))
    };
}
