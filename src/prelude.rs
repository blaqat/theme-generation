//! Crate preulde
pub use crate::*;
pub use anyhow;
pub use either::Either;
pub use notify::RecursiveMode;
pub use notify_debouncer_mini::{new_debouncer, DebouncedEvent};
pub use serde_json::{json, Map, Value};
pub use std::{
    collections::{HashMap, HashSet as Set},
    dbg as d,
    env::args,
    fmt::{self, Display, Write as _},
    fs::{read_dir, File},
    io::Write as _,
    path::{Path, PathBuf},
    println as p,
    str::FromStr,
};
pub use utils::*;

// Red error macro
#[macro_export]
macro_rules! error {
    ( $($args:expr),+ ) => {
        eprintln!("\x1b[31m{}\x1b[0m", format!($($args),+))
    };
}

#[macro_export]
macro_rules! ahh {
    ( $($args:expr),+) => {
        Err(Error::Processing(format!($($args),+)))
    };
}

#[macro_export]
macro_rules! dp {
    ( $($args:expr),+) => {
        d!($(format!("{}", $args)),+)
        // d!(format!($({})+, $($args),+))
    };
}

#[macro_export]
macro_rules! w {
    ( $w:expr, $($args:expr),+) => {
        writeln!(&mut $w, $($args),+).unwrap()
    };
}
