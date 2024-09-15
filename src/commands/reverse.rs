use crate::prelude::*;
use itertools::Itertools;
use steps::*;

/**
Reverse:
    Description:
        - Template + OriginalTheme = Variables
        - This generates a variable file by substituting values in the original theme file with variables in the template file.
        - This takes the OriginalTheme as the source of truth. Things in the template that arent in the OriginalTheme will be ignored.
        - The generated file will be saved in the current directory.
    Usage:
        substitutor rev template_file originalTheme [optional flags]
    Flags:
        -t int          Threshold for how many same colors to exist before adding to [colors] subgroup
        -o directory    Set output directory of variable file
        -n              Name of the output file
        -p path         Json Path to start the reverse process at
*/

pub const TOML_NULL: &str = "$none";

#[derive(PartialEq, Debug)]
enum ReverseFlags {
    Threshold(usize),
    OutputDirectory(PathBuf),
    Name(String),
    InnerPath(JsonPath),
}

#[derive(PartialEq, Debug)]
struct Flags {
    threshold: usize,          // Default to 3
    output_directory: PathBuf, // Default to current directory
    name: String,
    path: Option<JsonPath>,
}

impl ReverseFlags {
    fn into_vec(flags: Vec<String>) -> Result<Vec<Self>, Error> {
        flags.iter().map(|flag| Self::from_str(flag)).collect()
    }

    fn parse(flags: Vec<String>) -> Flags {
        let flags = Self::into_vec(flags).unwrap();
        let mut threshold = 3;
        let mut output_directory = PathBuf::from(".");
        let mut name = String::from("reversed-theme");
        let mut path = None;

        for flag in flags {
            match flag {
                Self::Threshold(value) => threshold = value,
                Self::OutputDirectory(path) => output_directory = path,
                Self::Name(n) => name = n,
                Self::InnerPath(p) => path = Some(p),
            }
        }

        Flags {
            threshold,
            output_directory,
            name,
            path,
        }
    }
}

impl FromStr for ReverseFlags {
    type Err = Error;

    fn from_str(flag: &str) -> Result<Self, Error> {
        match flag {
            flag if flag.starts_with("-p") => {
                let path = flag.split("=").last().unwrap();
                let path = JsonPath::from_str(path)
                    .map_err(|_| Error::InvalidFlag("reverse".to_owned(), flag.to_owned()))?;
                Ok(Self::InnerPath(path))
            }
            flag if flag.starts_with("-n") => {
                let name = flag.split("=").last().unwrap();
                Ok(Self::Name(name.to_owned()))
            }
            flag if flag.starts_with("-o") => {
                let path = flag.split("=").last().unwrap();
                let path = path.replace("~", std::env::var("HOME").unwrap().as_str());
                let path = Path::new(&path);
                if !path.exists() {
                    return Err(Error::InvalidFlag("reverse".to_owned(), flag.to_owned()));
                }
                Ok(Self::OutputDirectory(path.to_path_buf()))
            }
            flag if flag.starts_with("-t") => {
                let threshold = flag.split("=").last().unwrap();
                let threshold = threshold
                    .parse()
                    .map_err(|_| Error::InvalidFlag("reverse".to_owned(), flag.to_owned()))?;
                Ok(Self::Threshold(threshold))
            }
            _ => Err(Error::InvalidFlag("reverse".to_owned(), flag.to_owned())),
        }
    }
}

mod steps {
    use super::*;

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
            let name = var.name().to_string();
            var_set.safe_insert(&name, var);
        }

        for o in overrides {
            let name = o.name().to_string();
            unvar_set.safe_insert(&name, o);
        }

        let (pointers, unresolved_vars): (Vec<_>, Vec<_>) = var_set
            .get_unresolved()
            .into_iter()
            .partition(|v| v.is_pointer());

        type UnresolvedSet<'a> =
            HashMap<JsonPath, HashMap<ParsedValue, Vec<(String, &'a ResolvedVariable)>>>;

        let mut unresolved_set: UnresolvedSet = HashMap::new();
        for pointer in pointers {
            if let (var_name, ParsedValue::Variables(paths)) = (pointer.path, pointer.value) {
                for path in paths {
                    let unresolved_var = unresolved_vars
                        .iter()
                        .find(|v| v.path.to_string() == path)
                        .unwrap();

                    let identity = unresolved_var.identity();

                    unresolved_set
                        .entry(var_name.clone())
                        .or_default()
                        .entry(identity)
                        .or_default()
                        .push((path, unresolved_var));
                }
            }
        }

        // Identities
        for (var_name, iden_map) in &mut unresolved_set {
            let map = iden_map.clone();

            let identities = map.keys().collect::<Vec<_>>();
            let values = map.values().flatten().map(|(_, v)| v).collect::<Vec<_>>();

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
                    identity_of.iter().for_each(|v| {
                        iden_map
                            .entry((*identity).clone())
                            .or_default()
                            .push((v.path.to_string(), v));
                    });
                });

            let imv = iden_map
                .values()
                .map(|v| {
                    let mut v = v.clone();
                    v.dedup();
                    v
                })
                .collect::<Vec<_>>();

            let max_len = imv.iter().map(|v| v.len()).max().unwrap();
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
                // STILL A CHANCE!
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
                            unvar_set.insert(&next.name, new);
                            inserted = true;
                        }
                        Some(_) => current = new.clone(),
                        None => {
                            unvar_set.insert(&var_name.join(), new);
                            inserted = true;
                        }
                    }
                }
                if !inserted {
                    unvar_set.insert(&var_name.join(), (*u).clone());
                }
            }
        }

        var_set.resolve();

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
                for (key, val) in map1.iter() {
                    match map2.get(key) {
                        Some(val2) => {
                            let next_diff =
                                key_diff(val, val2, format!("{prefix}/{key}"), log_vars);
                            info.extend(next_diff)
                        }
                        _ => info.missing.push(format!("{prefix}/{key}")),
                    }
                }
            }

            (Value::Array(vec1), Value::Array(vec2)) => {
                for (key, val) in vec1.iter().enumerate() {
                    match vec2.get(key) {
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
                } else if log_vars && let (Value::String(str), val) = (val1, val2) {
                    info.parsed_vars
                        .push(SourcedVariable::new(prefix, str, val))
                }
            }

            (Value::String(str), val) | (val, Value::String(str)) => {
                if log_vars {
                    info.parsed_vars
                        .push(SourcedVariable::new(prefix, str, val))
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

    type ColorMap = HashMap<String, (String, Vec<Color>)>;
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
                    ParsedValue::String(format!("${}", name).replace("$color.", "@"))
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
                            ParsedValue::Color(_) => unreachable!(),
                            ParsedValue::Variables(_) => unreachable!(),
                        }
                    }
                    ParsedValue::Value(Value::Array(new_array))
                }
                Value::Object(o) => {
                    let mut new_obj = Map::new();
                    for (key, val) in o.iter() {
                        let replaced =
                            replace_color(&ParsedValue::Value(val.clone()), color_map, threshold);
                        match replaced {
                            ParsedValue::String(s) => {
                                new_obj.insert(key.to_owned(), Value::String(s))
                            }
                            ParsedValue::Value(v) => new_obj.insert(key.to_owned(), v),
                            ParsedValue::Null => new_obj.insert(key.to_owned(), Value::Null),
                            ParsedValue::Color(_) => unreachable!(),
                            ParsedValue::Variables(_) => unreachable!(),
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
        deletions: &Set<JsonPath>,
    ) -> Result<String, Error> {
        macro_rules! t {
            ($var_name:ident=$from:expr) => {
                let $var_name: toml::Value = {
                    match $from {
                        // Value::Null => toml::Value::String(String::from(TOML_NULL)),
                        // a => {
                        //     d!(&a);
                        //     serde_json::from_value(a).map_err(|json_err| {
                        //         Error::Processing(format!("Unhandeled theme json: {}", json_err))
                        //     })?
                        // }
                        a => into_toml(a)?,
                    }
                };
            };
        }

        // let grouped_toml: toml::Value = serde_json::from_value(grouped_json.clone())
        //     .map_err(|json_err| Error::Processing(format!("Invalid theme json: {}", json_err)))?;
        t!(grouped_toml = variables);

        let data = grouped_toml.as_table().unwrap();
        let mut doc = String::new();
        macro_rules! w {
            ($($args:expr),+) => {
                prelude::w!(doc, $($args),+)
            };
        }

        w!("# Reverse Generation Tool Version 3.0");
        // d!(data);
        for (k, v) in data
            .iter()
            .filter(|(_, v)| !matches!(v, toml::Value::Table(_)))
        {
            w!("{} = {}", k, v);
        }

        // d!(data);

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

        for (k, v) in data.iter().filter(|(k, _)| *k != "color") {
            if v.is_table() {
                w!("\n[{}]", k);
                for (k, v) in v.as_table().unwrap().iter() {
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

        w!("\n# Overrides");
        w!("[overrides]");
        // d!(&overrides);
        for (_, v) in overrides
            .to_map()
            .into_iter()
            .sorted_by_key(|(k, _)| k.clone())
        {
            // d!(&k, &v);
            t!(val = v.value.into_value());
            w!(r#""{}" = {}"#, v.path.join(), val);
        }

        w!("\n# Deletions");
        w!("[deletions]");
        w!("keys = [");
        for (i, d) in deletions
            .iter()
            .sorted_by(|a, b| a.to_string().cmp(&b.to_string()))
            .enumerate()
        {
            if i == deletions.len() - 1 {
                w!("\t\"{}\"", d);
            } else {
                w!("\t\"{}\",", d);
            }
        }
        w!("]");

        Ok(doc)
    }
}

pub fn reverse(
    template: ValidatedFile,
    theme: ValidatedFile,
    flags: Vec<String>,
) -> Result<(), Error> {
    let flags = ReverseFlags::parse(flags);

    // Step 1: Deserialize the template and theme files into Objects.
    let mut theme: Value = serde_json::from_reader(&theme.file)
        .map_err(|json_err| Error::Processing(format!("Invalid theme file json: {}", json_err)))?;
    let mut template: Value = serde_json::from_reader(&template.file).map_err(|json_err| {
        Error::Processing(format!("Invalid template file json: {}", json_err))
    })?;

    // Step 1.5: Traverse to the starting path if it exists.
    if let Some(starting_path) = flags.path {
        theme = starting_path
            .traverse(&theme)
            .map_err(|_| Error::Processing(String::from("Invalid starting path.")))?
            .clone();

        template = starting_path
            .traverse(&template)
            .map_err(|_| Error::Processing(String::from("Invalid starting path.")))?
            .clone();

        if !same_type(&theme, &template) {
            return Err(Error::Processing(String::from(
                "Starting path types do not match.",
            )));
        }
    }

    let reverse = |theme: Value, template: Value, file_name: String| -> Result<(), Error> {
        // Step 2: Built Data Structures (Deletions, Overrides, Variables, Colors)
        let var_diff = key_diff(&template, &theme, String::from(""), true);
        let override_diff = key_diff(&theme, &template, String::from(""), false);
        // d!(&var_diff, &override_diff);

        let overrides: Set<_> = override_diff
            .missing
            .iter()
            .chain(override_diff.collisions.iter())
            .map(|key| ResolvedVariable::from_path(key, &theme))
            .collect();

        let deletions: Set<_> = var_diff
            .missing
            .iter()
            .map(|key| key.parse::<JsonPath>().unwrap())
            .collect();

        // Step 3: Resolve Variables and Overrides
        let (variables, overrides) = resolve_variables(&var_diff, overrides);
        drop(var_diff);

        // Step 4: Build Color Redundancy Map & Replace Colors
        let color_map = to_color_map(&variables, &overrides);
        // d!(&color_map);

        // Step 5: Replace Colors In variables and overrides limited by threshold
        for (var_name, mut var) in variables.to_map().into_iter() {
            let val = replace_color(&var.value, &color_map, flags.threshold);
            // d!(&var_name, &val);
            var.value = val;
            variables.insert(&var_name, var.clone());
        }

        for (var_name, mut var) in overrides.to_map().into_iter() {
            let val = replace_color(&var.value, &color_map, flags.threshold);
            var.value = val;
            overrides.insert(&var_name, var.clone());
        }

        // Step 6: Add Colors to VariablesSet
        for (value, (color, v)) in color_map.iter() {
            if v.len() < flags.threshold {
                continue;
            }
            let var = ResolvedVariable::init(color, ParsedValue::String(value.to_owned()));
            variables.inc_insert(color, var);
        }
        drop(color_map);

        // Step 7: Create Groupings
        // e.g varname "a.b.c" = 1, "a.b.d" = 2 should be [a.b] = {c = 1, d = 2}
        let mut grouped_json = json!({});
        for (var_name, var) in variables.to_map().into_iter() {
            let split = var_name.rsplit_once('.');
            let path = if let Some((group, key)) = split {
                format!("{}/{}", group, key)
            } else {
                var_name.clone()
            }
            .parse::<JsonPath>()
            .unwrap();

            if let ParsedValue::Null = var.value {
                continue;
            }

            path.pave(&mut grouped_json, var.value.into_value())?;
        }

        // Step 8: Build the Toml Output
        let toml_output = generate_toml_string(grouped_json, &overrides, &deletions)
            .map_err(|e| Error::Processing(format!("Could not generate toml output: {:?}\nThis is probably indicative of needing to use the -p inner path", e)))?;
        let out_dir = flags.output_directory.clone();

        let mut out_file = out_dir.clone();
        let file_name = format!("{}.toml", file_name);
        out_file.push(file_name);

        let mut file = File::create(out_file)
            .map_err(|e| Error::Processing(format!("Could not create file: {}", e)))?;
        file.write_all(toml_output.as_bytes())
            .map_err(|e| Error::Processing(format!("Could not write to file: {}", e)))?;

        Ok(())
    };

    match (&theme, &template) {
        (Value::Object(_), Value::Object(_)) => {
            reverse(theme, template, flags.name)?;
        }
        (Value::Array(theme), Value::Array(template)) => {
            let template = template.first().unwrap();
            for (i, theme) in theme.iter().enumerate() {
                if !same_type(theme, template) {
                    return Err(Error::Processing(format!(
                        "Array index {} types do not match.",
                        i
                    )));
                }
                let default_name = "/name".parse::<JsonPath>().unwrap().traverse(theme).ok();
                let name = {
                    if let Some(name) = default_name {
                        name.as_str().unwrap().to_string()
                    } else {
                        format!("{}{}", flags.name, i)
                    }
                };
                reverse(theme.clone(), template.clone(), name)?;
            }
        }
        _ => return Err(Error::Processing(String::from("Invalid starting path."))),
    }

    Ok(())
}
