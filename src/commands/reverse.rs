use crate::prelude::*;
use commands::check::{parse_special_array, SpecialKey};
use itertools::Itertools;
use steps::{generate_toml_string, key_diff, replace_color, resolve_variables, to_color_map};

/**
Reverse:
    Description:
        - Template + `OriginalTheme` = Variables
        - This generates a variable file by substituting values in the original theme file with variables in the template file.
        - This takes the `OriginalTheme` as the source of truth. Things in the template that arent in th`OriginalTheme`me will be ignored.
        - The generated file will be saved in the current directory.
    Usage:
        substitutor rev `template_file` originalTheme [optional flags]
    Flags:
        -t int          Threshold for how many same colors to exist before adding to [colors] subgroup
        -o directory    Set output directory of variable file
        -n              Name of the output file
        -p path         Json Path to start the reverse process at
        -g[o|d]         Don't generate deletions or additions
*/

pub const TOML_NULL: &str = "$none";
pub const VALID_FLAGS: &[&str] = &["-t", "-o", "-n", "-p", "-g"];

#[derive(PartialEq, Debug)]
enum ReverseFlags {
    Threshold(usize),
    OutputDirectory(PathBuf),
    Name(String),
    InnerPath(JSPath),
    DontGenerate(Vec<char>),
}

#[allow(clippy::struct_excessive_bools)]
#[derive(PartialEq, Debug)]
struct Flags {
    threshold: usize,          // Default to 3
    output_directory: PathBuf, // Default to current directory
    name: String,
    path: Option<JSPath>,
    generate_deletions: bool,
    generate_additions: bool,
    generate_colors: bool,
    generate_manual: bool,
}

impl ReverseFlags {
    fn into_vec(flags: &[String]) -> Result<Vec<Self>, ProgramError> {
        flags.iter().map(|flag| Self::from_str(flag)).collect()
    }

    fn parse(flags: &[String]) -> Flags {
        let flags = Self::into_vec(flags).unwrap();
        let mut threshold = 3;
        let mut output_directory = PathBuf::from(".");
        let mut name = String::from("reversed-theme");
        let mut path = None;
        let mut generate_deletions = true;
        let mut generate_additions = true;
        let mut generate_colors = true;
        let mut generate_manual = true;

        for flag in flags {
            match flag {
                Self::Threshold(value) => threshold = value,
                Self::OutputDirectory(path) => output_directory = path,
                Self::Name(n) => name = n,
                Self::InnerPath(p) => path = Some(p),
                Self::DontGenerate(s) => {
                    generate_deletions = !s.contains(&'d');
                    generate_additions = !s.contains(&'o');
                    generate_colors = !s.contains(&'c');
                    generate_manual = !s.contains(&'p');
                }
            }
        }

        Flags {
            threshold,
            output_directory,
            name,
            path,
            generate_deletions,
            generate_additions,
            generate_colors,
            generate_manual,
        }
    }
}

impl FromStr for ReverseFlags {
    type Err = ProgramError;

    fn from_str(flag: &str) -> Result<Self, ProgramError> {
        match flag {
            flag if flag.starts_with("-p") => {
                let path = flag.split('=').last().unwrap();
                let path = JSPath::from_str(path).map_err(|_| {
                    ProgramError::InvalidFlag("reverse".to_owned(), flag.to_owned())
                })?;
                Ok(Self::InnerPath(path))
            }
            flag if flag.starts_with("-n") => {
                let name = flag.split('=').last().unwrap();
                Ok(Self::Name(name.to_owned()))
            }
            flag if flag.starts_with("-o") => {
                let path = flag.split('=').last().unwrap();
                let path = path.replace('~', std::env::var("HOME").unwrap().as_str());
                let path = Path::new(&path);
                if !path.exists() {
                    return Err(ProgramError::InvalidFlag(
                        "reverse".to_owned(),
                        flag.to_owned(),
                    ));
                }
                Ok(Self::OutputDirectory(path.to_path_buf()))
            }
            flag if flag.starts_with("-t") => {
                let threshold = flag.split('=').last().unwrap();
                let threshold = threshold.parse().map_err(|_| {
                    ProgramError::InvalidFlag("reverse".to_owned(), flag.to_owned())
                })?;
                Ok(Self::Threshold(threshold))
            }
            flag if flag.starts_with("-g") => {
                let chars = flag[1..].chars().collect();
                Ok(Self::DontGenerate(chars))
            }
            _ => Err(ProgramError::InvalidFlag(
                "reverse".to_owned(),
                flag.to_owned(),
            )),
        }
    }
}

mod steps {
    use super::*;
    type UnresolvedSet<'a> =
        HashMap<JSPath, HashMap<ParsedValue, Vec<(String, &'a ResolvedVariable)>>>;
    type ColorMap = HashMap<String, (String, Vec<Color>)>;

    #[allow(clippy::too_many_lines)]
    pub fn resolve_variables(
        var_diff: &KeyDiffInfo,
        overrides: Set<ResolvedVariable>,
    ) -> (VariableSet, VariableSet) {
        let res_var_diff = var_diff
            .parsed_vars
            .iter()
            .filter(|v| !v.variables.is_empty())
            .map(ResolvedVariable::from_src)
            .collect::<Vec<_>>();

        let var_set = VariableSet::new();
        let unvar_set = VariableSet::new();

        for var in res_var_diff {
            let name = var.name();
            var_set.safe_insert(&name, var);
        }

        for o in overrides {
            let name = o.name();
            unvar_set.safe_insert(&name, o);
        }

        let (pointers, unresolved_vars): (Vec<_>, Vec<_>) = var_set
            .get_unresolved()
            .into_iter()
            .partition(ResolvedVariable::is_pointer);

        let mut unresolved_set: UnresolvedSet = HashMap::new();
        for pointer in pointers {
            if let (var_name, ParsedValue::Variables(paths)) = (pointer.path, pointer.value) {
                for path in paths {
                    if let Some(unresolved_var) =
                        unresolved_vars.iter().find(|v| v.path.to_string() == path)
                    {
                        let identity = unresolved_var.identity();
                        unresolved_set
                            .entry(var_name.clone())
                            .or_default()
                            .entry(identity)
                            .or_default()
                            .push((path, unresolved_var));
                    } else if let Some(sib) = unresolved_vars
                        .iter()
                        .find_map(|v| v.siblings.iter().find(|s| s.path.to_string() == path))
                    {
                        unvar_set.inc_insert(&path, sib.clone());
                    }
                }
            }
        }

        // Identities
        for (var_name, iden_map) in &mut unresolved_set {
            let orig_map = iden_map.clone();

            let identities = orig_map.keys().collect::<Vec<_>>();
            let values = orig_map
                .values()
                .flatten()
                .map(|(_, v)| v)
                .collect::<Vec<_>>();

            identities
                .iter()
                .map(|identity| {
                    (
                        identity,
                        values
                            .iter()
                            .filter(|v| v.results_from(identity))
                            .collect::<Vec<_>>(),
                    )
                })
                .for_each(|(identity, identity_of)| {
                    for v in identity_of {
                        iden_map
                            .entry((*identity).clone())
                            .or_default()
                            .push((v.path.to_string(), v));
                    }
                });

            let imv = iden_map
                .values()
                .map(|v| {
                    let mut v = v.clone();
                    v.dedup();
                    v
                })
                .collect::<Vec<_>>();

            let max_len = imv.iter().map(Vec::len).max().unwrap();
            let (mut max, mut rest): (Vec<_>, Vec<_>) =
                imv.iter().partition(|v| v.len() == max_len);
            max.dedup();
            rest.dedup();

            let (first, max_rest) = (max.first().unwrap(), max.clone());
            let first_found = first.first().unwrap();
            let mut first_var = first_found.1.clone();

            let identity = iden_map
                .iter()
                .find(|(_, v)| v.contains(first_found))
                .unwrap()
                .0;
            first_var.value = identity.clone();
            first_var.next();

            var_set.insert(&var_name.join(), first_var);

            rest.extend(max_rest);
            let rest = rest
                .into_iter()
                .flatten()
                .filter(|v| !first.contains(v))
                .collect::<Vec<_>>();

            for (_, u) in &rest {
                let mut first = true;
                let mut inserted = false;

                let mut current = (*u).clone();
                while current.could_resolve() && !inserted {
                    let mut new = (*u).clone();
                    if first {
                        new.next();
                        first = false;
                    }
                    let mut new_new = new.clone();
                    new.next();
                    match new_new.next() {
                        Some(next) if !var_set.has_variable(&next.name) => {
                            unvar_set.inc_insert(&next.name, new);
                            inserted = true;
                        }
                        Some(_) => current = new.clone(),
                        None => {
                            unvar_set.inc_insert(&var_name.join(), new);
                            inserted = true;
                        }
                    }
                }

                if !inserted {
                    unvar_set.inc_insert(&var_name.join(), (*u).clone());
                }
            }
        }

        var_set.resolve();

        // Unresolve Null Mismatches
        let map = var_set.to_map();

        let siblings = map
            .iter()
            .flat_map(|(k, v)| v.siblings.iter().map(move |s| (k, s)))
            .filter(|(_, v)| v.value == ParsedValue::Null && v.variables.len() > 1);

        let filter = map
            .iter()
            .filter(|(_, v)| v.value == ParsedValue::Null && v.variables.len() > 1)
            .chain(siblings.clone())
            .map(|(k, v)| (k, v.to_owned()));

        for (var, mut val) in filter {
            let og = val.clone();
            while let Some(v) = val.next() {
                if !var_set.is_null(&v.name) {
                    unvar_set.inc_insert(var, og);
                    break;
                }
            }
        }

        (var_set, unvar_set)
    }

    pub fn key_diff(data1: &Value, data2: &Value, prefix: String, log_vars: bool) -> KeyDiffInfo {
        let mut info = KeyDiffInfo {
            missing: Vec::new(),
            collisions: Vec::new(),
            parsed_vars: Vec::new(),
        };

        match (data1, data2) {
            (Value::Object(map1), Value::Object(map2)) => {
                for (key, val) in map1 {
                    match map2.get(key) {
                        Some(val2) => {
                            let next_diff =
                                key_diff(val, val2, format!("{prefix}/{key}"), log_vars);
                            info.extend(next_diff);
                        }
                        _ => info.missing.push(format!("{prefix}/{key}")),
                    }
                }
            }

            (Value::Array(vec1), Value::Array(vec2)) => {
                let (is_vec1_spec, match_all1, spec_keys_1) = parse_special_array(vec1);
                let (is_vec2_spec, match_all2, spec_keys_2) = parse_special_array(vec2);
                let is_special = is_vec1_spec || is_vec2_spec;
                let match_all = match_all1 || match_all2;
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

                for (key, val) in vec1.iter().enumerate() {
                    let val2 = if is_special {
                        if !val.is_object() {
                            info.missing.push(format!("{prefix}/{key}"));
                            continue;
                        }
                        let found = vec2.iter().find(|val2| {
                            special_keys
                                .iter()
                                .map(|sp_key| {
                                    let val1_key = val.get(&sp_key.0).unwrap_or(&Value::Null);
                                    let val2_key = val2.get(&sp_key.0).unwrap_or(&Value::Null);
                                    sp_key.matches(val1_key, val2_key)
                                })
                                .reduce(|a, b| if match_all { a && b } else { a || b })
                                .unwrap_or_default()
                        });
                        if found.is_none() {
                            info.missing.push(format!("{prefix}/{key}"));
                            continue;
                        }
                        found
                    } else {
                        vec2.get(key)
                    };
                    match val2 {
                        Some(val2) => {
                            let next_diff =
                                key_diff(val, val2, format!("{prefix}/{key}"), log_vars);
                            info.extend(next_diff);
                        }
                        _ => info.missing.push(format!("{prefix}/{key}")),
                    }
                }
            }

            (val1, val2) if !log_vars && same_type(val1, val2) && val1 != val2 => {
                if !potential_set(val1, val2) {
                    info.collisions.push(prefix);
                }
            }

            (Value::String(str), val) | (val, Value::String(str)) => {
                if log_vars {
                    info.parsed_vars
                        .push(SourcedVariable::new(prefix, str, val));
                }
            }

            (val1, val2) if !log_vars && has_keys(val1) != has_keys(val2) => {
                info.collisions.push(prefix);
            }

            _ => (),
        }

        info
    }

    fn get_nested_values(j: &Value) -> Vec<Value> {
        match j {
            Value::Object(map) => map.values().flat_map(get_nested_values).collect(),
            Value::Array(vec) => vec.iter().flat_map(get_nested_values).collect(),
            val => vec![val.clone()],
        }
    }

    pub fn to_color_map(v: &VariableSet, o: &VariableSet) -> ColorMap {
        let mut color_map: ColorMap = HashMap::new();
        let get_num_matching_names =
            |n: &str, map: &ColorMap| map.values().filter(|(name, _)| name.starts_with(n)).count();

        let mut update_color_map = |col: &Color| {
            let mut name = match col.get_name().as_str() {
                "404" => format!("color.{}", color_map.keys().len()),
                s => s.to_owned(),
            };

            name = match get_num_matching_names(&name, &color_map) {
                0 => name,
                n => format!("{}{}", name, n + 1),
            };

            let colors = color_map.entry(col.to_alphaless_hex()).or_default();
            if colors.0.is_empty() {
                colors.0 = name;
            }
            colors.1.push(col.clone());
        };

        v.to_vec()
            .iter()
            .chain(o.to_vec().iter())
            .for_each(|var| match var.value {
                ParsedValue::Color(ref col) => {
                    update_color_map(col);
                }
                ParsedValue::String(ref s) if let Ok(ref col) = s.parse::<Color>() => {
                    update_color_map(col);
                }
                ParsedValue::Value(ref v) => match v {
                    value if has_keys(value) => {
                        let values = get_nested_values(value);
                        for val in values {
                            match val {
                                Value::String(ref s) if let Ok(ref col) = s.parse::<Color>() => {
                                    update_color_map(col);
                                }
                                _ => (),
                            }
                        }
                    }
                    Value::String(s) if let Ok(ref col) = s.parse::<Color>() => {
                        update_color_map(col);
                    }
                    _ => (),
                },
                _ => (),
            });

        color_map
    }

    pub fn replace_color(val: &ParsedValue, color_map: &ColorMap, threshold: usize) -> ParsedValue {
        let get_color = |c: &Color| {
            let hex = c.to_alphaless_hex();
            let (name, v) = color_map.get(&hex).unwrap();
            if v.len() >= threshold {
                if c.has_alpha() {
                    ParsedValue::String(
                        format!("${}..{}", name, c.get_alpha()).replace("$color.", "@"),
                    )
                } else {
                    ParsedValue::String(format!("${name}").replace("$color.", "@"))
                }
            } else {
                ParsedValue::String(c.to_string())
            }
        };

        match val {
            ParsedValue::Color(ref col) => get_color(col),
            ParsedValue::String(ref s) if let Ok(ref col) = s.parse::<Color>() => get_color(col),
            ParsedValue::Value(ref v) => match v {
                Value::Array(a) => {
                    let mut new_array = Vec::new();
                    for val in a {
                        let replaced =
                            replace_color(&ParsedValue::Value(val.clone()), color_map, threshold);
                        match replaced {
                            ParsedValue::String(s) => new_array.push(Value::String(s)),
                            ParsedValue::Value(v) => new_array.push(v),
                            ParsedValue::Null => new_array.push(Value::Null),
                            ParsedValue::Color(_) | ParsedValue::Variables(_) => unreachable!(),
                        }
                    }
                    ParsedValue::Value(Value::Array(new_array))
                }
                Value::Object(o) => {
                    let mut new_obj = Map::new();
                    for (key, val) in o {
                        let replaced =
                            replace_color(&ParsedValue::Value(val.clone()), color_map, threshold);
                        match replaced {
                            ParsedValue::String(s) => {
                                new_obj.insert(key.to_owned(), Value::String(s))
                            }
                            ParsedValue::Value(v) => new_obj.insert(key.to_owned(), v),
                            ParsedValue::Null => new_obj.insert(key.to_owned(), Value::Null),
                            ParsedValue::Color(_) | ParsedValue::Variables(_) => unreachable!(),
                        };
                    }
                    ParsedValue::Value(Value::Object(new_obj))
                }
                Value::String(s) => {
                    replace_color(&ParsedValue::String(s.to_owned()), color_map, threshold)
                }
                _ => val.clone(),
            },
            _ => val.clone(),
        }
    }

    /// Order:
    /// 1. Top Level Variables
    /// 2. Color Variables
    /// 3. Grouped Variables
    /// 4. Overrides
    /// 5. Deletions
    pub fn generate_toml_string(
        variables: Value,
        overrides: &VariableSet,
        deletions: &Set<JSPath>,
        flags: &Flags,
    ) -> Result<String, ProgramError> {
        macro_rules! t {
            ($var_name:ident=$from:expr) => {
                let $var_name: toml::Value = {
                    match $from {
                        a => into_toml(a)?,
                    }
                };
            };
        }

        t!(grouped_toml = variables);

        let data = grouped_toml.as_table().unwrap();
        let mut doc = String::new();
        macro_rules! w {
            ($($args:expr),+) => {
                prelude::w!(doc, $($args),+)
            };
        }

        w!("# Reverse Generation Tool Version 3.0");
        for (k, v) in data
            .iter()
            .filter(|(_, v)| !matches!(v, toml::Value::Table(_)))
        {
            w!("{} = {}", k, v);
        }

        if flags.generate_colors {
            w!("\n# Theme Colors");
            w!("[color]");
            for (_, v) in data.iter().filter(|(k, _)| *k == "color") {
                for (color, value) in v
                    .as_table()
                    .unwrap()
                    .iter()
                    .sorted_by(|(a, _), (b, _)| a.cmp(b))
                {
                    w!("{} = {}", color, value);
                }
            }
        }

        for (k, v) in data.iter().filter(|(k, _)| *k != "color") {
            if v.is_table() {
                w!("\n[{}]", k);
                for (k, v) in v.as_table().unwrap() {
                    match v {
                        toml::Value::Array(a) => {
                            w!("{} = [", k);
                            for (i, v) in a.iter().enumerate() {
                                if i == a.len() - 1 {
                                    w!("\t{}", v);
                                } else {
                                    w!("\t{},", v);
                                }
                            }
                            w!("]");
                        }
                        _ => w!("{} = {}", k, v),
                    }
                }
            }
        }

        if flags.generate_additions {
            w!("\n# Overrides");
            w!("[overrides]");
            for (_, v) in overrides
                .to_map()
                .into_iter()
                .sorted_by_key(|(k, _)| k.clone())
            {
                t!(val = v.value.clone().into_value());
                w!(r#""{}" = {}"#, v.path.join(), val);
            }
        }

        if flags.generate_deletions {
            w!("\n# Deletions");
            w!("[deletions]");
            w!("keys = [");
            for (i, d) in deletions
                .iter()
                .sorted_by(|a, b| match (a.has_num_in_path(), b.has_num_in_path()) {
                    (true, true) => b.to_string().cmp(&a.to_string()),
                    _ => a.to_string().cmp(&b.to_string()),
                })
                .enumerate()
            {
                if i == deletions.len() - 1 {
                    w!("\t\"{}\"", d);
                } else {
                    w!("\t\"{}\",", d);
                }
            }
            w!("]");
        }

        Ok(doc)
    }
}

#[allow(clippy::too_many_lines)]
pub fn reverse(
    template: &ValidatedFile,
    theme: &ValidatedFile,
    flags: &[String],
) -> Result<(), ProgramError> {
    let flags = ReverseFlags::parse(flags);
    let mut generated_files = Vec::new();

    // Step 1: Deserialize the template and theme files into Objects.
    let mut theme: Value = serde_json::from_reader(&theme.file).map_err(|json_err| {
        ProgramError::Processing(format!("Invalid theme file json: {json_err}"))
    })?;
    let mut template: Value = serde_json::from_reader(&template.file).map_err(|json_err| {
        ProgramError::Processing(format!("Invalid template file json: {json_err}"))
    })?;

    // Step 1.5: Traverse to the starting path if it exists.
    if let Some(starting_path) = &flags.path {
        theme = starting_path
            .traverse(&theme)
            .map_err(|_| ProgramError::Processing(String::from("Invalid starting path.")))?
            .clone();

        template = starting_path
            .traverse(&template)
            .map_err(|_| ProgramError::Processing(String::from("Invalid starting path.")))?
            .clone();

        if !same_type(&theme, &template) {
            return Err(ProgramError::Processing(String::from(
                "Starting path types do not match.",
            )));
        }
    }

    let reverse = |theme: Value,
                   template: Value,
                   file_name: &str,
                   gen_color: bool|
     -> Result<String, ProgramError> {
        // Step 1.5: Preprocesser Overrides
        // When a key starts with $::, it means variable should = the value
        // e.g
        // $::colors.red = "#FF0000"
        // results in toml:
        // [colors]
        // red = "#FF0000"
        let mut preproc_ignore_keys = Vec::new();
        let preproc_overrides = match &template {
            Value::Object(map) => map
                .iter()
                .filter_map(|(k, v)| {
                    if k.starts_with("$::") {
                        preproc_ignore_keys.push(format!("/{k}"));
                        Some((k.strip_prefix("$::").unwrap(), v.clone()))
                    } else {
                        None
                    }
                })
                .collect(),
            _ => HashMap::new(),
        };

        // Step 2: Built Data Structures (Deletions, Overrides, Variables, Colors)
        let var_diff = key_diff(&template, &theme, String::new(), true);
        let override_diff = key_diff(&theme, &template, String::new(), false);

        let overrides: Set<_> = override_diff
            .missing
            .iter()
            .chain(override_diff.collisions.iter())
            .filter(|key| !preproc_ignore_keys.contains(key))
            .map(|key| ResolvedVariable::from_path(key, &theme))
            .collect();

        let mut deletions: Set<_> = var_diff
            .missing
            .iter()
            .filter(|key| !preproc_ignore_keys.contains(key))
            .map(|key| key.parse::<JSPath>().unwrap())
            .collect();

        // Step 3: Resolve Variables and Overrides
        let (variables, overrides) = resolve_variables(&var_diff, overrides);
        drop(var_diff);

        // Step 4: Build Color Redundancy Map & Replace Colors
        let color_map = to_color_map(&variables, &overrides);

        // Step 5: Replace Colors In variables and overrides limited by threshold
        if gen_color {
            for (var_name, mut var) in variables.to_map() {
                let val = replace_color(&var.value, &color_map, flags.threshold);
                var.value = val;
                variables.insert(&var_name, var.clone());
            }

            for (var_name, mut var) in overrides.to_map() {
                let val = replace_color(&var.value, &color_map, flags.threshold);
                var.value = val;
                overrides.insert(&var_name, var.clone());
            }

            // Step 6: Add Colors to VariablesSet
            for (value, (color, v)) in &color_map {
                if v.len() < flags.threshold {
                    continue;
                }
                let var = ResolvedVariable::init(color, ParsedValue::String(value.to_owned()));
                variables.inc_insert(color, var);
            }
            drop(color_map);
        }

        let get_var_path = |var_name: String| -> JSPath {
            let split = var_name.rsplit_once('.');

            if let Some((group, key)) = split {
                format!("{group}/{key}")
            } else {
                var_name
            }
            .parse::<JSPath>()
            .unwrap()
        };

        // Step 7: Create Groupings
        // e.g varname "a.b.c" = 1, "a.b.d" = 2 should be [a.b] = {c = 1, d = 2}
        let mut grouped_json = json!({});
        for (var_name, var) in variables.to_map() {
            if var.value == ParsedValue::Null {
                continue;
            }

            get_var_path(var_name).pave(&mut grouped_json, var.value.into_value())?;
        }

        if flags.generate_manual {
            for (var_name, val) in preproc_overrides {
                match var_name {
                    "color" if flags.generate_colors => match val {
                        Value::Object(ref obj) => {
                            for (color, value) in obj {
                                let color = format!("color/{color}");
                                get_var_path(color).pave(&mut grouped_json, value.clone())?;
                            }
                        }
                        _ => {
                            return Err(ProgramError::Processing(format!(
                                "Invalid $::color value: {val:?}\nExpected an object with colors: {{color: Value}}\nAlternative run with flag -gc to ignore colors"
                            )));
                        }
                    },
                    "deletions" if flags.generate_deletions => match val {
                        Value::Array(keys) => {
                            deletions.extend(
                                keys.iter()
                                    .map(|v| v.as_str().unwrap().parse::<JSPath>().unwrap()),
                            );
                        }
                        Value::Object(ref obj)
                            if let Some(keys) = obj.get("keys")
                                && let Value::Array(keys) = keys =>
                        {
                            deletions.extend(
                                keys.iter()
                                    .map(|v| v.as_str().unwrap().parse::<JSPath>().unwrap()),
                            );
                        }
                        _ => {
                            return Err(ProgramError::Processing(format!(
                                "Invalid $::deletions value: {val:?}\nExpected an array of strings or an object with keys: []\nAlternative run with flag -gd to ignore deletions"
                            )));
                        }
                    },
                    "overrides" if flags.generate_additions => match val {
                        Value::Object(map) => {
                            for (k, v) in map {
                                overrides.insert(&k, ResolvedVariable::init_override(&k, &v));
                            }
                        }
                        _ => {
                            return Err(ProgramError::Processing(format!(
                                    "Invalid $::overrides value: {val:?}\nExpected an object\nAlternative run with flag -ga to ignore overrides"
                                )));
                        }
                    },
                    _ => {
                        get_var_path(var_name.to_owned()).pave(&mut grouped_json, val)?;
                    }
                }
            }
        }

        // Step 8: Build the Toml Output
        let toml_output = generate_toml_string(grouped_json, &overrides, &deletions, &flags)
            .map_err(|e| ProgramError::Processing(format!("Could not generate toml output: {e:?}\nThis is probably indicative of needing to use the -p inner path")))?;
        let out_dir = flags.output_directory.clone();

        let mut out_file = out_dir;
        let file_name = format!("{file_name}.toml");
        out_file.push(file_name.clone());

        let mut file = File::create(out_file)
            .map_err(|e| ProgramError::Processing(format!("Could not create file: {e}")))?;
        file.write_all(toml_output.as_bytes())
            .map_err(|e| ProgramError::Processing(format!("Could not write to file: {e}")))?;

        Ok(file_name)
    };

    match (&theme, &template) {
        (Value::Object(_), Value::Object(_)) => {
            generated_files.push(reverse(
                theme,
                template,
                &flags.name,
                flags.generate_colors,
            )?);
        }
        (Value::Array(theme), Value::Array(template)) => {
            let template = template.first().unwrap();
            for (i, theme) in theme.iter().enumerate() {
                if !same_type(theme, template) {
                    return Err(ProgramError::Processing(format!(
                        "Array index {i} types do not match."
                    )));
                }
                let default_name = "/name".parse::<JSPath>().unwrap().traverse(theme).ok();
                let name = {
                    if let Some(name) = default_name
                        && let Some(name) = name.as_str()
                    {
                        name.to_string()
                    } else {
                        format!("{}{}", flags.name, i)
                    }
                };
                generated_files.push(reverse(
                    theme.clone(),
                    template.clone(),
                    &name,
                    flags.generate_colors,
                )?);
            }
        }
        _ => {
            return Err(ProgramError::Processing(String::from(
                "Invalid starting path.",
            )))
        }
    }

    println!(
        "Reversed into ({}) files: {:?}",
        generated_files.len(),
        generated_files,
    );

    Ok(())
}
