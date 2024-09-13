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
pub fn from_stow_cmd() -> Result<String, ExitCode> {
    let mut output = "".to_string();
    let dotfiles_dir = match dotfiles::get_dotfiles_path() {
        Ok(path) => path,
        Err(e) => {
            output.push_str(&e);
            return Err(ReturnCode::NoSetupFolder.into());
        }
    };

    // --- Getting user confirmation ---
    io::stdout().flush().unwrap();

    let mut answer = String::new();
    io::stdin().read_line(&mut answer).unwrap();
    if !matches!(answer.trim().to_lowercase().as_str(), "yes" | "y") {
        return Ok("User did not aceept confirmation".into());
    }

    // --- initializing required directory ---
    let configs_path = dotfiles_dir.join("Configs");
    fs::create_dir_all(&configs_path).expect("Could not create required directory.");

    // --- Moving dotfiles to Configs/ ---
    let cwd = match fs::read_dir(&dotfiles_dir) {
        Ok(dir) => dir,
        Err(e) => { output.push_str("Could not open current directory"); return Err(ExitCode::FAILURE);}
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

    Ok(output)
}

/// Creates the necessary files and folders for a tuckr directory if they don't exist
pub fn init_cmd() -> Result<String, ExitCode> {
    let mut output = "".to_string();
    macro_rules! create_dirs {
        ($($dirname: expr),+) => {
            $(
            if let Err(e) = fs::create_dir_all($dirname) {
                output.push_str(&e.to_string());
                return Err(ExitCode::FAILURE);
            })+
        };
    }

    let dotfiles_dir = if cfg!(test) {
        dotfiles::get_dotfiles_path().unwrap()
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

    Ok(output)
}

pub fn push_cmd(group: String, files: &[String]) -> Result<String, ExitCode> {
    let mut output = "".to_string();
    let dotfiles_dir = match dotfiles::get_dotfiles_path() {
        Ok(dir) => dir.join("Configs").join(group),
        Err(e) => {
            output.push_str(&e);
            return Err(ReturnCode::CouldntFindDotfiles.into());
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
            
            std::io::stdout().flush().unwrap();
            let mut confirmation = String::new();
            std::io::stdin().read_line(&mut confirmation).unwrap();

            let confirmed = matches!(confirmation.trim().to_lowercase().as_str(), "y" | "yes");

            if confirmed {
                fs::create_dir_all(target_dir).unwrap();
                fs::copy(file, target_file).unwrap();
            }
        }
    }

    if any_file_failed {
        Err(ReturnCode::NoSuchFileOrDir.into())
    } else {
        Ok(output)
    }
}

pub fn pop_cmd(groups: &[String]) -> Result<String, ExitCode> {
    let mut output = "".to_string();
    let dotfiles_dir = match dotfiles::get_dotfiles_path() {
        Ok(dir) => dir.join("Configs"),
        Err(e) => {
            output.push_str(&e);
            return Err(ReturnCode::CouldntFindDotfiles.into());
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

        return Err(ReturnCode::NoSuchFileOrDir.into());
    }

    output.push_str("The following groups will be removed:");
    for group in groups {
        output.push_str("\t");
        output.push_str(group);
    }
    std::io::stdout().flush().unwrap();
    let mut confirmation = String::new();
    std::io::stdin().read_line(&mut confirmation).unwrap();

    let confirmed = matches!(confirmation.trim().to_lowercase().as_str(), "y" | "yes");

    if !confirmed {
        return Ok(output);
    }

    for group_path in valid_groups {
        fs::remove_dir_all(group_path).unwrap();
    }

    Ok(output)
}

pub fn ls_hooks_cmd() -> Result<String, ExitCode> {
    let mut output = "".to_string();
    let dir = match dotfiles::get_dotfiles_path() {
        Ok(dir) => dir.join("Hooks"),
        Err(err) => {
            output.push_str(&err);
            return Err(ReturnCode::CouldntFindDotfiles.into());
        }
    };

    if !dir.exists() {
        output.push_str("There's no directory setup for Hooks");
        return Err(ReturnCode::NoSetupFolder.into());
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

    let true_symbol = "✓".green().to_string();
    let false_symbol = "✗".red().to_string();

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
        return Ok(output);
    }

    use tabled::{Margin, Style};

    let mut hooks_list = Table::new(rows);
    hooks_list
        .with(Style::rounded())
        .with(Margin::new(4, 4, 1, 1))
        .with(Modify::new(Segment::new(1.., 1..)).with(Alignment::center()));
    output.push_str(&hooks_list.to_string());

    Ok(output)
}

// todo: make ls-secrets command prettier
pub fn ls_secrets_cmd() -> Result<String, ExitCode> {
    let mut output = "".to_string();
    let secrets_dir = match dotfiles::get_dotfiles_path() {
        Ok(p) => p.join("Secrets"),
        Err(e) => { output.push_str(&e); return Ok(output); },
    };

    let Ok(secrets) = secrets_dir.read_dir() else {
        return Err(ReturnCode::NoSetupFolder.into());
    };

    for secret in secrets {
        let secret = secret.unwrap();
        println!("{}", secret.file_name().to_str().unwrap());
    }
    Ok(output)
}

pub fn groupis_cmd(files: &[String]) -> Result<String, ExitCode> {
    let mut output: String = "".into();
    let dotfiles_dir = match dotfiles::get_dotfiles_path() {
        Ok(path) => path,
        Err(e) => {
            output.push_str(&e);
            return Err(ReturnCode::NoSetupFolder.into());
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
            eprintln!("{}", format!("`{file} does not exist.`").red());
            continue;
        }

        if let Ok(dotfile) = dotfiles::Dotfile::try_from(file_path.clone()) {
            println!("{}", dotfile.group_name);
            continue;
        }

        while !file_path.is_symlink() {
            if !file_path.pop() {
                output.push_str(&format!("`{file}` is not a tuckr dotfile."));
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
                    eprintln!("{err}");
                    continue;
                }
            };

            println!("{}", dotfile.group_name);

            return Ok(dotfile.group_name);
        }
    }
    Ok(output)
}
