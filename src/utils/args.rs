use crate::prelude::*;
// const DEFAULT_EDIT_DIRECTORY: &str = "~/.config/theme-substitutor/";

#[derive(Debug, Clone)]
enum FileType {
    Theme,
    Template,
    Variable,
}

#[derive(Debug)]
pub struct ValidatedFile {
    pub format: String,
    pub file: File,
    pub name: String,
    file_type: FileType,
}

impl Clone for ValidatedFile {
    fn clone(&self) -> Self {
        let new_file = File::open(&self.name).unwrap_or_else(|_| {
            panic!("Error opening file (File Moved or Deleted): {}", &self.name);
        });
        Self {
            format: self.format.clone(),
            file: new_file,
            name: self.name.clone(),
            file_type: self.file_type.clone(),
        }
    }
}

impl ValidatedFile {
    fn from_str(file_path: &str) -> Result<Self, ProgramError> {
        let path = Path::new(&file_path);

        let format = if path.ends_with("template.json") {
            String::from("template")
        } else {
            path.extension()
                .and_then(|e| e.to_str())
                .ok_or_else(|| ProgramError::InvalidFile(String::from(file_path)))?
                .to_owned()
        };

        let file_type = match format.as_str() {
            "json" => FileType::Theme,
            "toml" => FileType::Variable,
            "template" => FileType::Template,
            _ => return Err(ProgramError::InvalidIOFormat(format)),
        };

        let file = File::open(file_path)
            .map_err(|_| ProgramError::InvalidFile(String::from(file_path)))?;

        let name = file_path.to_owned();

        Ok(Self {
            format,
            file,
            name,
            file_type,
        })
    }

    fn all_variable_files(source_directory: &Path) -> Result<Vec<Self>, ProgramError> {
        // Variable files are toml files.
        let mut files = Vec::new();

        for entry in source_directory.read_dir().map_err(|_| {
            ProgramError::InvalidFile(String::from(source_directory.to_str().unwrap()))
        })? {
            let entry = entry.map_err(|_| {
                ProgramError::InvalidFile(String::from(source_directory.to_str().unwrap()))
            })?;
            let path = entry.path();
            let path_str = path
                .to_str()
                .ok_or_else(|| ProgramError::InvalidFile(String::from(path.to_str().unwrap())))?;
            if path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
            {
                files.push(Self::from_str(path_str)?);
            }
        }

        Ok(files)
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub enum ValidCommands {
    Check,
    Generate,
    Reverse,
    Help,
    Watch,
    Edit,
    New,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProgramError {
    NoCommand,
    InvalidCommand,
    HelpAll,
    NotEnoughArguments(ValidCommands),
    InvalidFile(String),
    InvalidFileType,
    InvalidFlag(String, String),
    InvalidIOFormat(String),
    HelpInvalidCommand,
    Processing(String),
}

impl ValidCommands {
    fn from_str(command: &str) -> Result<Self, ProgramError> {
        match command {
            "check" => Ok(Self::Check),
            "gen" => Ok(Self::Generate),
            "rev" => Ok(Self::Reverse),
            "help" => Ok(Self::Help),
            "watch" => Ok(Self::Watch),
            "edit" => Ok(Self::Edit),
            "new" => Ok(Self::New),
            _ => Err(ProgramError::InvalidCommand),
        }
    }

    pub fn list_commands() -> Vec<&'static str> {
        vec!["check", "gen", "rev", "help", "watch", "edit", "new"]
    }
}

fn get_generation_files(
    flags: &[String],
    command_args: &[String],
    call_dir: PathBuf,
) -> Result<(PathBuf, ValidatedFile, Vec<ValidatedFile>), ProgramError> {
    let directory = if flags.iter().any(|flag| flag.starts_with("-i")) {
        let flags = commands::generate::FlagTypes::parse(flags)?;
        flags.directory()
    } else {
        call_dir
    };

    let (template_file, variable_files) = match (command_args[0].as_str(), command_args[1].as_str())
    {
        ("all", template_file) | (template_file, "all") => {
            let template_file = ValidatedFile::from_str(template_file)?;
            if matches!(template_file.file_type, FileType::Template) {
                let variable_files = ValidatedFile::all_variable_files(&directory)?;
                (template_file, variable_files)
            } else {
                return Err(ProgramError::InvalidFileType);
            }
        }
        (template_file, variable_file) => {
            let files = (
                ValidatedFile::from_str(template_file)?,
                ValidatedFile::from_str(variable_file)?,
            );

            match (&files.0.file_type, &files.1.file_type) {
                (FileType::Template, FileType::Variable) => (files.0, vec![files.1]),
                (FileType::Variable, FileType::Template) => (files.1, vec![files.0]),
                _ => return Err(ProgramError::InvalidFileType),
            }
        }
    };

    Ok((directory, template_file, variable_files))
}

pub fn run_command(args: Vec<String>) -> Result<(), ProgramError> {
    if args.len() < 2 {
        return Err(ProgramError::NoCommand);
    }

    let call_dir = std::env::current_dir().unwrap();

    let mut flags: Vec<_> = args
        .iter()
        .filter(|&x| x.starts_with('-'))
        .map(std::string::ToString::to_string)
        .collect();

    flags.sort();
    flags.dedup();

    let command = ValidCommands::from_str(&args[1])?;

    let command_args: Vec<_> = args
        .into_iter()
        .skip(2)
        .filter(|x| !x.starts_with('-'))
        .collect();

    match command {
        ValidCommands::Help if command_args.is_empty() => Err(ProgramError::HelpAll),
        ValidCommands::Help => {
            let help_command = ValidCommands::from_str(&command_args[0])
                .map_err(|_| ProgramError::HelpInvalidCommand)?;
            commands::help(&help_command);
            Ok(())
        }
        ValidCommands::New => {
            let theme_name = &command_args[0];
            commands::new(theme_name, &flags)
        }
        command if command_args.len() < 2 => Err(ProgramError::NotEnoughArguments(command)),
        ValidCommands::Check => {
            let file1 = ValidatedFile::from_str(&command_args[0])?;
            let file2 = ValidatedFile::from_str(&command_args[1])?;
            commands::check(&file1, &file2)
        }
        ValidCommands::Reverse => {
            let template_file = ValidatedFile::from_str(&command_args[0])?;
            let theme_file = ValidatedFile::from_str(&command_args[1])?;

            match (&template_file.file_type, &theme_file.file_type) {
                (FileType::Template, FileType::Theme) => {
                    commands::reverse(&template_file, &theme_file, &flags)
                }
                (FileType::Theme, FileType::Template) => {
                    commands::reverse(&theme_file, &template_file, &flags)
                }
                _ => Err(ProgramError::InvalidFileType),
            }
        }
        ValidCommands::Generate => {
            let (_, template_file, variable_files) =
                get_generation_files(&flags, &command_args, call_dir)?;

            commands::generate(&template_file, variable_files, &flags)
        }

        ValidCommands::Watch => {
            let (directory, template_file, variable_files) =
                get_generation_files(&flags, &command_args, call_dir)?;

            commands::watch(&directory, &template_file, &variable_files, &flags)
        }

        ValidCommands::Edit => {
            let template_file = ValidatedFile::from_str(&command_args[0])?;
            let theme_file = ValidatedFile::from_str(&command_args[1])?;
            let watch_flags: Vec<_> = flags
                .clone()
                .into_iter()
                .filter(|x| commands::generate::VALID_FLAGS.contains(&&x[0..2]))
                .collect();
            let reverse_flags: Vec<_> = flags
                .into_iter()
                .filter(|x| commands::reverse::VALID_FLAGS.contains(&&x[0..2]))
                .filter(|x| &x[0..2] != "-o") // Edit should reverse to the currend directory
                .collect();
            let watch_command = |name| {
                vec!["", "watch", name, "all"]
                    .into_iter()
                    .map(String::from)
                    .chain(watch_flags.clone())
                    .collect::<Vec<String>>()
            };

            match (&template_file.file_type, &theme_file.file_type) {
                (FileType::Template, FileType::Theme) => {
                    commands::reverse(&template_file, &theme_file, &reverse_flags)?;
                    run_command(watch_command(&template_file.name))
                }
                (FileType::Theme, FileType::Template) => {
                    commands::reverse(&theme_file, &template_file, &reverse_flags)?;
                    run_command(watch_command(&theme_file.name))
                }
                _ => Err(ProgramError::InvalidFileType),
            }
        }
    }
}
