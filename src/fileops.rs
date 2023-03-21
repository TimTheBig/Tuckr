//! Creates basic file structure for tuckr
//!
//! Contains functions to create the base directories and to convert users from stow to tuckr

use owo_colors::OwoColorize;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path;
use std::process::ExitCode;

/// Converts a stow directory into a tuckr directory
pub fn from_stow_cmd() -> Result<(), ExitCode> {
    print!("{}", "Are you sure you want to convert the current directory to tuckr?\nAll files starting with a dot will be ignored (y/N) ".yellow());
    io::stdout().flush().unwrap();

    let mut answer = String::new();
    io::stdin().read_line(&mut answer).unwrap();
    let answer = answer.to_lowercase().trim().to_owned();

    if let "yes" | "y" = answer.as_str()  {
        return Ok(());
    }

    init_cmd()?;

    let cwd = env::current_dir().unwrap();
    let curr_path = cwd.to_str().unwrap();
    let cwd = fs::read_dir(&cwd).expect("Could not open current directory");
    const IGNORED_FILES: &[&str] = &["COPYING", "LICENSE", "README.md"];

    for dir in cwd {
        let dir = dir.unwrap();
        let dirname = dir.file_name().to_str().unwrap().to_owned();
        if dirname.starts_with('.') || IGNORED_FILES.contains(&dirname.as_str()) {
            continue;
        }

        let path = path::PathBuf::from(curr_path)
            .join("Configs")
            .join(&dirname);

        if !dirname.ends_with("Configs")
            && !dirname.ends_with("Hooks")
            && !dirname.ends_with("Secrets")
        {
            fs::rename(dir.path().to_str().unwrap(), path).expect("Could not move files");
        }
    }

    Ok(())
}

/// Creates the necessary files and folders for a tuckr directory if they don't exist
pub fn init_cmd() -> Result<(), ExitCode> {
    if let Err(e) = fs::create_dir("Configs") {
        eprintln!("{}", e.red());
    }

    if let Err(e) = fs::create_dir("Hooks") {
        eprintln!("{}", e.red());
    }

    if let Err(e) = fs::create_dir("Secrets") {
        eprintln!("{}", e.red());
    }

    Ok(())
}

pub fn ls_hooks_cmd() -> Result<(), ExitCode> {
    todo!()
}

pub fn ls_secrets_cmd() -> Result<(), ExitCode> {
    todo!()
}
