//! Creates basic file structure for tuckr
//!
//! Contains functions to create the base directories and to convert users from stow to tuckr

use crate::dotfiles::{self, ReturnCode};
use owo_colors::OwoColorize;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{fs, path};
use tabled::object::Segment;
use tabled::{Alignment, Modify, Table, Tabled};

pub fn dir_map<F>(dir_path: impl AsRef<Path>, mut func: F)
where
    F: FnMut(&Path),
{
    let dir_path = dir_path.as_ref();
    let dir = match fs::read_dir(dir_path) {
        Ok(f) => f,
        Err(_) => panic!("{} does not exist", dir_path.to_str().unwrap()),
    };

    let mut queue: Vec<path::PathBuf> = dir.map(|f| f.unwrap().path()).collect();

    while let Some(curr_file) = queue.pop() {
        func(&curr_file);

        if curr_file.is_dir() {
            for dir in fs::read_dir(curr_file).unwrap() {
                let dir = dir.unwrap();
                queue.push(dir.path());
            }
        }
    }
}

/// Converts a stow directory into a tuckr directory
pub fn from_stow_cmd() -> (String, ExitCode) {
    let mut output = "".to_string();
    let dotfiles_dir = match dotfiles::get_dotfiles_path(&mut output) {
        Ok(path) => path,
        Err(e) => {
            output.push_str(&e.to_string());
            return (output, ReturnCode::NoSetupFolder.into());
        }
    };

    // --- initializing required directory ---
    let configs_path = dotfiles_dir.join("Configs");
    fs::create_dir_all(&configs_path).expect("Could not create required directory.");

    // --- Moving dotfiles to Configs/ ---
    let cwd = match fs::read_dir(&dotfiles_dir) {
        Ok(dir) => dir,
        Err(e) => { output.push_str("Could not open current directory"); return (output, ExitCode::FAILURE);}
    };

    for file in cwd {
        let dir = file.unwrap();
        if !dir.metadata().unwrap().is_dir() {
            continue;
        }

        let dirname = dir.file_name().to_str().unwrap().to_owned();
        if dirname.starts_with('.') {
            continue;
        }

        let path = configs_path.join(&dirname);

        if !dirname.ends_with("Configs")
            && !dirname.ends_with("Hooks")
            && !dirname.ends_with("Secrets")
        {
            fs::rename(dir.path(), path).expect("Could not move files");
        }
    }

    (output, ExitCode::SUCCESS)
}

/// Creates the necessary files and folders for a tuckr directory if they don't exist
pub fn init_cmd() -> (String, ExitCode) {
    let mut output = "".to_string();
    macro_rules! create_dirs {
        ($($dirname: expr),+) => {
            $(
            if let Err(e) = fs::create_dir_all($dirname) {
                output.push_str(&e.to_string());
                return (output, ExitCode::FAILURE);
            })+
        };
    }

    let dotfiles_dir = if cfg!(test) {
        dotfiles::get_dotfiles_path(&mut output).unwrap()
    } else {
        dirs::config_dir().unwrap().join("dotfiles")
    };

    create_dirs!(
        dotfiles_dir.join("Configs"),
        dotfiles_dir.join("Hooks"),
        dotfiles_dir.join("Secrets")
    );

    output.push_str( &format!(
            "A dotfiles directory has been created on `{}`.",
            dotfiles_dir.to_str().unwrap()
        )
    );

    (output, ExitCode::SUCCESS)
}

pub fn push_cmd(group: String, files: &[String]) -> (String, ExitCode) {
    let mut output = "".to_string();
    let dotfiles_dir = match dotfiles::get_dotfiles_path(&mut output) {
        Ok(dir) => dir.join("Configs").join(group),
        Err(e) => {
            output.push_str(&e.to_string());
            return (output, ReturnCode::CouldntFindDotfiles.into());
        }
    };

    let mut any_file_failed = false;
    for file in files {
        let file = PathBuf::from(file);
        if !file.exists() {
            output.push_str(file.to_str().unwrap_or(""));
            output.push_str(" does not exist.");
            any_file_failed = true;
            continue;
        }

        let file = path::absolute(file).unwrap();
        let target_file = dotfiles_dir.join(dotfiles::get_target_basepath(&file));
        let target_dir = target_file.parent().unwrap();

        if !target_file.exists() {
            fs::create_dir_all(target_dir).unwrap();

            if cfg!(target_family = "unix") || file.is_file() {
                fs::copy(file, target_file).unwrap();
            } else {
                dir_map(file, |f| {
                    let file = path::absolute(f).unwrap();
                    let target_file = dotfiles_dir.join(dotfiles::get_target_basepath(&file));
                    fs::create_dir_all(target_file.parent().unwrap()).unwrap();
                    fs::copy(file, target_file).unwrap();
                });
            }
        } else {            
            fs::create_dir_all(target_dir).unwrap();
            fs::copy(file, target_file).unwrap();
        }
    }

    if any_file_failed {
        (output, ReturnCode::NoSuchFileOrDir.into())
    } else {
        (output, ExitCode::SUCCESS)
    }
}

pub fn pop_cmd(groups: &[String]) -> (String, ExitCode) {
    let mut output = "".to_string();
    let dotfiles_dir = match dotfiles::get_dotfiles_path(&mut output) {
        Ok(dir) => dir.join("Configs"),
        Err(e) => {
            output.push_str(&e.to_string());
            return (output, e.into());
        }
    };

    let mut valid_groups = Vec::new();
    let mut invalid_groups = Vec::new();
    for group in groups {
        let group_dir = dotfiles_dir.join(group);
        if !group_dir.is_dir() {
            invalid_groups.push(group);
            continue;
        }

        if !group_dir.exists() {
            invalid_groups.push(group);
        } else {
            valid_groups.push(group_dir);
        }
    }

    if !invalid_groups.is_empty() {
        for group in invalid_groups {
            output.push_str(group);
            output.push_str(" does not exist.");
        }

        return (output, ReturnCode::NoSuchFileOrDir.into());
    }

    output.push_str("The following groups will be removed:");
    for group in groups {
        output.push_str("\t");
        output.push_str(group);
    }

    for group_path in valid_groups {
        fs::remove_dir_all(group_path).unwrap();
    }

    (output, ExitCode::SUCCESS)
}

pub fn ls_hooks_cmd() -> (String, ExitCode) {
    let mut output = "".to_string();
    let dir = match dotfiles::get_dotfiles_path(&mut output) {
        Ok(dir) => dir.join("Hooks"),
        Err(e) => {
            output.push_str(&e.to_string());
            return (output, ReturnCode::CouldntFindDotfiles.into());
        }
    };

    if !dir.exists() {
        output.push_str("There's no directory setup for Hooks");
        return (output, ReturnCode::NoSetupFolder.into());
    }

    #[derive(Tabled)]
    struct ListRow<'a> {
        #[tabled(rename = "Group")]
        group: String,
        #[tabled(rename = "Prehook")]
        prehook: &'a str,
        #[tabled(rename = "Posthook")]
        posthook: &'a str,
    }

    let dir = fs::read_dir(dir).unwrap();
    let mut rows = Vec::new();

    let true_symbol = "✓".to_string();
    let false_symbol = "✗".to_string();

    for hook in dir {
        let hook_dir = hook.unwrap();
        let hook_name = hook_dir.file_name();
        let group = hook_name.to_str().unwrap().to_string();

        let mut hook_entry = ListRow {
            group,
            prehook: &false_symbol,
            posthook: &false_symbol,
        };

        for hook in fs::read_dir(hook_dir.path()).unwrap() {
            let hook = hook.unwrap().file_name();
            let hook = hook.to_str().unwrap();
            if hook.starts_with("pre") {
                hook_entry.prehook = &true_symbol;
            } else if hook.starts_with("post") {
                hook_entry.posthook = &true_symbol;
            }
        }

        rows.push(hook_entry);
    }

    if rows.is_empty() {
        output.push_str("No hooks have been set up yet.");
        return (output, ExitCode::SUCCESS);
    }

    use tabled::{Margin, Style};

    let mut hooks_list = Table::new(rows);
    hooks_list
        .with(Style::rounded())
        .with(Margin::new(4, 4, 1, 1))
        .with(Modify::new(Segment::new(1.., 1..)).with(Alignment::center()));
    output.push_str(&hooks_list.to_string());

    (output, ExitCode::SUCCESS)
}

// todo: make ls-secrets command prettier
pub fn ls_secrets_cmd() -> (String, ExitCode) {
    let mut output = "".to_string();
    let secrets_dir = match dotfiles::get_dotfiles_path(&mut output) {
        Ok(p) => p.join("Secrets"),
        Err(e) => { output.push_str(&e.to_string()); return (output, e.into()); },
    };

    let Ok(secrets) = secrets_dir.read_dir() else {
        return (output, ReturnCode::NoSetupFolder.into());
    };

    for secret in secrets {
        let secret = secret.unwrap();
        output.push_str(secret.file_name().to_str().unwrap());
    }
    (output, ExitCode::SUCCESS)
}

pub fn groupis_cmd(files: &[String]) -> (String, ExitCode) {
    let mut output: String = "".into();
    let dotfiles_dir = match dotfiles::get_dotfiles_path(&mut output) {
        Ok(path) => path,
        Err(e) => {
            output.push_str(&e.to_string());
            return (output, ReturnCode::NoSetupFolder.into());
        }
    }
    .join("Configs");

    let groups: Vec<_> = dotfiles_dir
        .read_dir()
        .unwrap()
        .filter_map(|f| {
            let f = f.unwrap();
            if f.file_type().unwrap().is_dir() {
                Some(f.file_name().into_string().unwrap())
            } else {
                None
            }
        })
        .collect();

    'next_file: for file in files {
        let mut file_path = PathBuf::from(file);
        if !file_path.exists() {
            output.push_str(&file);
            output.push_str(" does not exist.");
            continue;
        }

        if let Ok(dotfile) = dotfiles::Dotfile::try_from(file_path.clone()) {
            output.push_str(&dotfile.group_name);
            continue;
        }

        while !file_path.is_symlink() {
            if !file_path.pop() {
                output.push_str(&file);
                output.push_str(" is not a tuckr dotfile.");
                break 'next_file;
            }
        }

        let basepath = dotfiles::get_target_basepath(&file_path);

        for group in &groups {
            let dotfile_path = dotfiles_dir.join(group).join(&basepath);

            if !dotfile_path.exists() {
                continue;
            }

            let dotfile = match dotfiles::Dotfile::try_from(dotfile_path) {
                Ok(dotfile) => dotfile,
                Err(err) => {
                    output.push_str(&err);
                    continue;
                }
            };

            output.push_str(&dotfile.group_name);

            return (output, ExitCode::SUCCESS);
        }
    }
    (output, ExitCode::SUCCESS)
}
