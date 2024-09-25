//! Crate preulde
pub use crate::*;

/// External crates
pub use anyhow;
pub use either::Either;
pub use notify::RecursiveMode;
pub use notify_debouncer_mini::new_debouncer;
pub use serde_json::{json, Map, Value};

/// Standard library
pub use std::{
    collections::{HashMap, HashSet as Set},
    env::args,
    fmt::{self, Display, Write as _},
    fs::File,
    io::Write as _,
    path::{Path, PathBuf},
    println as p,
    str::FromStr,
};

/// Internal modules
pub use utils::*;

/// Macros
#[macro_export]
macro_rules! error {
    ( $($args:expr),+ ) => {
        eprintln!("\x1b[31m{}\x1b[0m", format!($($args),+))
    };
}

#[macro_export]
macro_rules! ahh {
    ( $($args:expr),+) => {
        Err(ProgramError::Processing(format!($($args),+)))
    };
}

#[macro_export]
macro_rules! w {
    ( $w:expr, $($args:expr),+) => {
        writeln!(&mut $w, $($args),+).unwrap()
    };
}
