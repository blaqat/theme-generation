use crate::prelude::*;
/// Watch Mode:
///     Description:
///         - Watch changes to .toml files in a directory or a specific file and generate the theme file on each change.
///         - This makes it better to see live changes fast as you are making a theme
///     Usage:
///         substitutor watch templateFile variableFile|all [optional flags]
///     Flags:
///         -p path         Inner path to the theme in the theme file
///         -o directory    Set output directory of generatedTheme
///         -n name         Set name of output theme file
///         -i directory    Set directory where the .toml files are located

// commands::watch(&mut directory, template_file, variable_files, flags)
pub fn watch(
    directory: PathBuf,
    template_file: ValidatedFile,
    variable_files: Vec<ValidatedFile>,
    flags: Vec<String>,
) -> Result<(), Error> {
    // d!(&directory);
    let (tx, rx) = std::sync::mpsc::channel();
    let mut debouncer = new_debouncer(std::time::Duration::from_millis(100), tx)
        .map_err(|e| Error::Processing(String::from("Error creating notify watcher.")))?;

    let mut watcher = debouncer.watcher();

    // d!(variable_files);

    for file in &variable_files {
        let mut path = directory.clone();
        path.push(&file.name);
        // d!(&path);

        watcher
            .watch(&path, RecursiveMode::Recursive)
            .map_err(|e| Error::Processing(String::from("Error watching file.")))?;
    }

    loop {
        match rx.try_recv() {
            Ok(ref event) if let Ok(ref event) = event => {
                let variable_files = variable_files.iter().map(|v| v.clone()).collect();
                commands::generate(template_file.clone(), variable_files, flags.clone())?;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                // No events to process, continue the loop
            }
            Ok(_) => {}
            Err(e) => {
                error!("watch error: {:?}", e);
            }
        }
    }
}
