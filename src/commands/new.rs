/**
New:
    Description:
        - Generates a new project with a theme and its variants
    Usage:
        substitutor new `theme-name` [flags]
    Flags:
        -o directory: path      Set output directory of variable file
        -t template: path       Set template file to use
        -T themes: path[]       Set paths of custom light and dark themes with trailing :d/:l to differentiaate
        -s style: str           Set style of template to use (dark or light)
        -v variants: str[]      Names of theme variants to auto fill
                                - Optionally end string with :d or :l to use dark or light style
        -d description: str     Description of theme
*/
use crate::prelude::*;
use std::{fs, path::PathBuf, process::Command};

static DEFAULT_TEMPLATE: &str = "templates/new-hls.json.template";

#[derive(Debug)]
struct ThemeFile(PathBuf, ThemeStyle);

impl FromStr for ThemeFile {
    type Err = ProgramError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        let path = PathBuf::from(parts[0]);

        if !path.exists() {
            return Err(ProgramError::Processing(format!(
                "Theme file does not exist: {}",
                path.to_str().unwrap()
            )));
        }

        let style = if parts.len() < 2 {
            ThemeStyle::Dark
        } else {
            parts[1].parse()?
        };

        Ok(Self(path, style))
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
enum ThemeStyle {
    Dark,
    Light,
}

impl FromStr for ThemeStyle {
    type Err = ProgramError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            s if s.starts_with('d') => Ok(Self::Dark),
            s if s.starts_with('l') => Ok(Self::Light),
            _ => Err(ProgramError::InvalidFlag(
                "new".to_owned(),
                format!("Invalid style: {s}"),
            )),
        }
    }
}

#[derive(Debug, Clone)]
struct Variant {
    names: ThemeNames,
    style: ThemeStyle,
}

impl FromStr for Variant {
    type Err = ProgramError;

    /*
     * Parses a string in the format "name:style" where:
     * - `name` is the name of the theme variant
     * - `style` is optional and can be either 'd' for dark or 'l' for light.
     */
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split(':').collect();
        let name = parts[0].parse()?;
        let style = match parts.get(1).and_then(|s| s.chars().next()) {
            Some('l') => ThemeStyle::Light,
            _ => ThemeStyle::Dark,
        };
        Ok(Self { names: name, style })
    }
}

#[derive(Debug)]
struct Flags {
    output_directory: PathBuf,
    template: PathBuf,
    style: ThemeStyle,
    variants: Vec<Variant>,
    description: String,
    themes: Vec<ThemeFile>,
}

#[derive(Debug)]
enum FlagTypes {
    OutputDirectory(PathBuf),
    Template(PathBuf),
    Style(ThemeStyle),
    Variants(Vec<Variant>),
    Description(String),
    Themes(Vec<ThemeFile>),
}

impl FromStr for FlagTypes {
    type Err = ProgramError;

    fn from_str(flag: &str) -> Result<Self, ProgramError> {
        let get_directory = |path: &str| -> Result<PathBuf, ProgramError> {
            let path = path.replace('~', std::env::var("HOME").unwrap().as_str());
            let path = Path::new(&path);
            if !path.exists() {
                return Err(ProgramError::Processing(format!(
                    "Invalid file/directory: {}",
                    path.to_str().unwrap()
                )));
            }
            Ok(path.to_path_buf())
        };
        match flag {
            flag if flag.starts_with("-o") => {
                let path = flag.split('=').next_back().unwrap();
                get_directory(path).map(Self::OutputDirectory)
            }
            flag if flag.starts_with("-t") => {
                let path = flag.split('=').next_back().unwrap();
                Ok(Self::Template(get_directory(path)?))
            }
            flag if flag.starts_with("-T") => {
                let paths = flag.split('=').next_back().unwrap();
                let themes = paths
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.parse().unwrap())
                    .collect();
                Ok(Self::Themes(themes))
            }
            flag if flag.starts_with("-s") => {
                let style = flag.split('=').next_back().unwrap();
                Ok(Self::Style(style.parse()?))
            }
            flag if flag.starts_with("-v") => {
                let variants = flag.split('=').next_back().unwrap();
                let variants = variants
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| s.parse().unwrap())
                    .collect();
                Ok(Self::Variants(variants))
            }
            flag if flag.starts_with("-d") => {
                let description = flag.split('=').next_back().unwrap();
                Ok(Self::Description(description.to_owned()))
            }
            _ => Err(ProgramError::InvalidFlag(
                "reverse".to_owned(),
                flag.to_owned(),
            )),
        }
    }
}

impl FlagTypes {
    fn into_vec(flags: &[String]) -> Result<Vec<Self>, ProgramError> {
        flags.iter().map(|flag| Self::from_str(flag)).collect()
    }

    pub fn parse(flags: &[String]) -> Result<Flags, ProgramError> {
        let flags = Self::into_vec(flags)?;
        let mut output_directory = PathBuf::from(".");
        let mut template = None;
        let mut style = ThemeStyle::Dark;
        let mut variants = Vec::new();
        let mut description = String::from("This is a theme made for zed.");
        let mut themes = Vec::new();
        for flag in flags {
            match flag {
                Self::OutputDirectory(path) => output_directory = path,
                Self::Template(template_path) => template = Some(template_path),
                Self::Style(style_name) => style = style_name,
                Self::Variants(variants_list) => variants = variants_list,
                Self::Description(desc) => description = desc,
                Self::Themes(theme_files) => themes = theme_files,
            }
        }

        if template.is_none() {
            let default_template: PathBuf =
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PathBuf::from(DEFAULT_TEMPLATE));
            template = Some(default_template);
        }

        if themes.is_empty() {
            themes.push(ThemeFile(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join(PathBuf::from("themes/theme-dark.json")),
                ThemeStyle::Dark,
            ));
            themes.push(ThemeFile(
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join(PathBuf::from("themes/theme-light.json")),
                ThemeStyle::Light,
            ));
        }

        Ok(Flags {
            output_directory,
            template: template.unwrap(),
            style,
            variants,
            description,
            themes,
        })
    }
}

#[derive(Debug, Clone)]
struct ThemeNames {
    name: String,
    dash_case: String,
}

impl FromStr for ThemeNames {
    type Err = ProgramError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let dash_case = s.to_lowercase().replace(' ', "-");
        Ok(Self {
            name: s.to_string(),
            dash_case,
        })
    }
}

mod steps {
    use super::{
        fs, Command, HashMap, Path, ProgramError, ThemeFile, ThemeNames, ThemeStyle, Variant,
    };
    use tera::{Context, Tera};

    /// Creates a new project directory by copying the project template to the specified path.
    pub fn create_project_directory(
        path: &Path,
        templates_path: &Path,
    ) -> Result<(), ProgramError> {
        if path.exists() {
            return Err(ProgramError::Processing(format!(
                "Output directory already exists: {}. Cannot create new project.",
                path.to_str().unwrap()
            )));
        }

        if cfg!(windows) {
            Command::new("Copy-Item")
                .args([
                    "-Path",
                    templates_path.join("project").to_str().unwrap(),
                    "-Destination",
                    path.to_str().unwrap(),
                    "-Recurse",
                ])
                .output()
                .map_err(|e| {
                    ProgramError::Processing(format!(
                        "Error copying project template to output directory: {e}"
                    ))
                })?;
        } else {
            Command::new("cp")
                .args([
                    "-r",
                    templates_path.join("project").to_str().unwrap(),
                    path.to_str().unwrap(),
                ])
                .output()
                .map_err(|e| {
                    ProgramError::Processing(format!(
                        "Error copying project template to output directory: {e}"
                    ))
                })?;
        }

        Ok(())
    }

    /// Generates a preview string for the README by rendering each variant using a Tera template.
    fn generate_preview_str(variants: &[Variant]) -> String {
        static README_PREVIEW_TEMPLATE: &str = r#"
### {{title}}
<img src="assets/{{dash}}.png" width="670">"#;
        let mut template = Tera::default();
        template
            .add_raw_template("preview", README_PREVIEW_TEMPLATE)
            .unwrap();

        variants
            .iter()
            .map(|v| {
                let mut preview_ctx = Context::new();
                preview_ctx.insert("title", &v.names.name);
                preview_ctx.insert("dash", &v.names.dash_case);
                template.render("preview", &preview_ctx).unwrap()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Updates the README.md file with the theme name, description, variants, and previews.
    pub fn update_readme(
        path: &Path,
        names: &ThemeNames,
        variants: &[Variant],
        description: &str,
    ) -> Result<(), ProgramError> {
        let previews = generate_preview_str(variants);

        let mut readme_ctx = Context::new();
        readme_ctx.insert("theme_name", &names.name);
        readme_ctx.insert("theme_title", &names.name);
        readme_ctx.insert("theme_dash", &names.dash_case);
        readme_ctx.insert("theme_description", description);
        readme_ctx.insert("theme_previews", &previews);
        readme_ctx.insert(
            "theme_variants",
            &variants
                .iter()
                .map(|v| v.names.name.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        );

        let readme_str = fs::read_to_string(path)
            .map_err(|e| ProgramError::Processing(format!("Error reading README.md: {e}")))?;
        let readme_str = Tera::one_off(&readme_str, &readme_ctx, false)
            .map_err(|e| ProgramError::Processing(format!("Error rendering README.md: {e}")))?;

        fs::write(path, readme_str)
            .map_err(|e| ProgramError::Processing(format!("Error writing to README.md: {e}")))?;

        Ok(())
    }

    /// Updates the extension.toml file with the theme name and description.
    pub fn update_extensions_toml(
        path: &Path,
        names: &ThemeNames,
        description: &str,
    ) -> Result<(), ProgramError> {
        let mut extension_tempalte = Tera::default();

        let mut extension_toml_ctx = Context::new();
        extension_toml_ctx.insert("theme_dash", &names.dash_case);
        extension_toml_ctx.insert("theme_title", &names.name);
        extension_toml_ctx.insert("theme_description", description);

        let extension_toml_str = fs::read_to_string(path)
            .map_err(|e| ProgramError::Processing(format!("Error reading extension.toml: {e}")))?;
        let extension_toml_str = extension_tempalte
            .render_str(&extension_toml_str, &extension_toml_ctx)
            .map_err(|e| {
                ProgramError::Processing(format!("Error rendering extension.toml: {e}"))
            })?;

        fs::write(path, extension_toml_str).map_err(|e| {
            ProgramError::Processing(format!("Error writing to extension.toml: {e}"))
        })?;

        Ok(())
    }

    /// Generates the JSON strings for each theme variant by rendering the appropriate theme file with Tera.
    fn generate_variants_json(
        theme_files: &[ThemeFile],
        variants: &[Variant],
    ) -> Result<String, ProgramError> {
        let mut cache: HashMap<ThemeStyle, String> = HashMap::new();
        let mut get_theme = |style: &ThemeStyle| -> Result<String, ProgramError> {
            if let Some(json) = cache.get(style) {
                return Ok(json.clone());
            }

            let json_path = theme_files
                .iter()
                .find(|f| &f.1 == style)
                .unwrap()
                .0
                .clone();

            let json = fs::read_to_string(&json_path).map_err(|e| {
                ProgramError::Processing(format!("error reading {}: {e}", json_path.display()))
            })?;

            cache.insert(style.clone(), json.clone());
            Ok(json)
        };

        Ok(variants
            .iter()
            .map(|v| -> Result<String, ProgramError> {
                let mut ctx = Context::new();
                ctx.insert("theme_name", &v.names.name);
                Tera::one_off(&get_theme(&v.style)?, &ctx, false)
                    .map_err(|e| ProgramError::Processing(e.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?
            .join(",\n\t\t"))
    }

    /// Updates the themes/theme.json file with the new theme name and generates a new theme file.
    pub fn update_theme_json(
        path: &Path,
        theme_files: &[ThemeFile],
        names: &ThemeNames,
        variants: &[Variant],
    ) -> Result<(), ProgramError> {
        let theme_json_path = path.join("themes/theme.json");
        let themes = generate_variants_json(theme_files, variants)?;

        let mut theme_ctx = Context::new();
        theme_ctx.insert("theme_name", &names.dash_case);
        theme_ctx.insert("themes", &themes);

        let theme_json_str = fs::read_to_string(&theme_json_path)
            .map_err(|e| ProgramError::Processing(format!("Error reading theme.json: {e}")))?;
        let theme_json_str = Tera::one_off(&theme_json_str, &theme_ctx, false)
            .map_err(|e| ProgramError::Processing(format!("Error rendering theme.json: {e}")))?;
        let theme_json_new_path = path.join(format!("themes/{}.json", &names.dash_case));
        fs::remove_file(&theme_json_path)
            .map_err(|e| ProgramError::Processing(format!("Error removing theme.json: {e}")))?;
        fs::write(&theme_json_new_path, theme_json_str).map_err(|e| {
            ProgramError::Processing(format!("Error writing to {}.json: {}", &names.dash_case, e))
        })?;

        Ok(())
    }
}

/// Creates a new theme project with the given name and optional flags.
pub fn new(name: &str, flags: &[String]) -> Result<(), ProgramError> {
    let mut flags = FlagTypes::parse(flags)?;
    let theme_name: ThemeNames = name.parse()?;

    flags.variants.insert(
        0,
        Variant {
            names: theme_name.clone(),
            style: flags.style,
        },
    );

    let templates_directory: PathBuf =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(PathBuf::from("templates"));
    if !templates_directory.exists() {
        return Err(ProgramError::Processing(format!(
            "Project Template directory does not exist: {}. Cannot create new project.",
            templates_directory.to_str().unwrap()
        )));
    }

    // 1. Clone Project Template into Output Directory Using System Commands
    let output_directory = flags.output_directory.join(&theme_name.dash_case);
    steps::create_project_directory(&output_directory, &templates_directory)?;

    // 2. Update Template Files with Theme Content
    let extension_toml_path = output_directory.join("extension.toml");
    let readme_path = output_directory.join("README.md");

    steps::update_readme(
        &readme_path,
        &theme_name,
        &flags.variants,
        &flags.description,
    )?;

    steps::update_extensions_toml(&extension_toml_path, &theme_name, &flags.description)?;

    steps::update_theme_json(
        &output_directory,
        &flags.themes,
        &theme_name,
        &flags.variants,
    )?;

    // 3. Copy Template File to Output Directory/templates
    let template_path = flags.template;
    let new_template_path = output_directory
        .join("templates")
        .join(template_path.file_name().unwrap());
    if !new_template_path.exists() {
        fs::create_dir_all(new_template_path.parent().unwrap()).map_err(|e| {
            ProgramError::Processing(format!("Error creating directory for template file: {e}"))
        })?;
    }
    fs::copy(&template_path, &new_template_path).map_err(|e| {
        ProgramError::Processing(format!(
            "Error copying template file to output directory: {e}"
        ))
    })?;

    Ok(())
}
