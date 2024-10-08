//! Manages script running
//!
//! Hooks are run in a state machine.
//! Hooking steps:
//! 1. Setup scripts are run
//! 2. Dotfiles are symlinked
//! 3. Post setup scripts are run

use crate::dotfiles::{self, Dotfile, ReturnCode};
use crate::symlinks;
use owo_colors::OwoColorize;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, ExitCode};

/// Prints a single row info box with title on the left
/// and content on the right
fn print_info_box(title: &str, content: &str, output: &mut String) -> String {
    let mut hook_box = tabled::builder::Builder::default()
        .set_columns([title])
        .add_record([content])
        .to_owned()
        .build();
    hook_box
        .with(tabled::Rotate::Left)
        .with(tabled::Style::rounded().off_vertical());
    output.push_str(&hook_box.to_string());

    hook_box.to_string()
}

#[derive(Debug, PartialEq)]
enum DeployStep {
    Initialize, // Default value before starting deployment
    PreHook,
    Symlink,
    PostHook,
}

/// State machine for running hooks
struct DeployStages(DeployStep);

impl DeployStages {
    fn new() -> DeployStages {
        DeployStages(DeployStep::Initialize)
    }
}

impl Iterator for DeployStages {
    type Item = DeployStep;

    fn next(&mut self) -> Option<DeployStep> {
        match self.0 {
            DeployStep::Initialize => {
                self.0 = DeployStep::PreHook;
                Some(DeployStep::PreHook)
            }
            DeployStep::PreHook => {
                self.0 = DeployStep::Symlink;
                Some(DeployStep::Symlink)
            }
            DeployStep::Symlink => {
                self.0 = DeployStep::PostHook;
                Some(DeployStep::PostHook)
            }
            DeployStep::PostHook => None,
        }
    }
}

/// Runs hooks of type PreHook or PostHook
fn run_hook(group: &str, hook_type: DeployStep, output: &mut String) -> Result<String, ExitCode> {
    let dotfiles_dir = match dotfiles::get_dotfiles_path(output) {
        Ok(dir) => dir,
        Err(e) => {
            output.push_str(&e.to_string());
            return Err(ReturnCode::CouldntFindDotfiles.into());
        }
    };

    let group_dir = PathBuf::from(&dotfiles_dir).join("Hooks").join(group);
    let mut str_output = "".to_string();
    let Ok(group_dir) = fs::read_dir(group_dir) else {
        str_output.push_str("Could not read Hooks, folder may not exist or does not have the appropriate permissions");
        return Err(ReturnCode::NoSetupFolder.into());
    };

    for file in group_dir {
        let file = file.unwrap().path();
        let filename = file.file_name().unwrap().to_str().unwrap();
        let file = file.to_str().unwrap();
        // make sure it will only run for their specific hooks
        match hook_type {
            DeployStep::PreHook => {
                if !filename.starts_with("pre") {
                    continue;
                }
                print_info_box("Running Prehook", group.to_string().as_str(), output);
            }

            DeployStep::PostHook => {
                if !filename.starts_with("post") {
                    continue;
                }
                print_info_box("Running Posthook", group.to_string().as_str(), output);
            }
            _ => (),
        }

        let mut output = match Command::new(file).spawn() {
            Ok(output) => output,
            Err(e) => {
                str_output.push_str(&e.to_string());
                return Err(ExitCode::FAILURE);
            }
        };

        if !output.wait().unwrap().success() {
            print_info_box(
                "Failed to hook".to_string().as_str(),
                format!("{group} {filename}").as_str(),
                &mut str_output,
            );
            return Err(ExitCode::FAILURE);
        }
    }

    Ok(str_output)
}

/// Runs hooks for specified groups
pub fn set_cmd(
    groups: &[String],
    exclude: &[String],
    force: bool,
    adopt: bool,
) -> (String, ExitCode) {
    let mut output = "".to_string();
    if let Some(invalid_groups) =
        dotfiles::check_invalid_groups(dotfiles::DotfileType::Hooks, groups, &mut output)
    {
        for group in invalid_groups {
            output.push_str(&group);
            output.push_str(" does not exist.");
        }

        return (output, ReturnCode::NoSuchFileOrDir.into());
    }

    let run_deploy_steps = |step: DeployStages, group: Dotfile, output: &mut String| -> Result<(), ExitCode> {
        if !group.is_valid_target() {
            output.push_str("Not a valid targit");
            return Err(ExitCode::FAILURE);
        }

        for i in step {
            match i {
                DeployStep::Initialize => return Ok(()),

                DeployStep::PreHook => {
                    run_hook(&group.group_name, DeployStep::PreHook, output)?;
                }

                DeployStep::Symlink => {
                    print_info_box(
                        "Symlinking group",
                        group.group_name.to_string().as_str(),
                        output,
                    );
                    output.push_str(&symlinks::add_cmd(groups, exclude, force, adopt).0);
                }

                DeployStep::PostHook => { run_hook(&group.group_name, DeployStep::PostHook, output)?; },
            }
        }

        Ok(())
    };

    let hooks_dir = match dotfiles::get_dotfiles_path(&mut output) {
        Ok(dir) => dir.join("Hooks"),
        Err(e) => {
            output.push_str(&e.to_string());
            return (output, ReturnCode::NoSetupFolder.into());
        }
    };

    if groups.contains(&'*'.to_string()) {
        for folder in fs::read_dir(hooks_dir).unwrap() {
            let folder = folder.unwrap().path();
            let Ok(group) = Dotfile::try_from(folder.clone()) else {
                output.push_str("Got an invalid group: ");
                output.push_str(folder.to_str().unwrap_or(""));
                return (output, ExitCode::FAILURE);
            };
            match run_deploy_steps(DeployStages::new(), group, &mut output) {
                Ok(()) => return (output, ExitCode::SUCCESS),
                Err(e) => return (output, e),
            };
        }

        return (output, ExitCode::SUCCESS);
    }

    for group in groups {
        let hook_path = hooks_dir.join(group);
        let Ok(group) = Dotfile::try_from(hook_path.clone()) else {
            output.push_str("Got an invalid group: ");
            output.push_str(hook_path.to_str().unwrap_or(""));
            return (output, ExitCode::FAILURE);
        };
        match run_deploy_steps(DeployStages::new(), group, &mut output) {
            Ok(()) => return (output, ExitCode::SUCCESS),
            Err(e) => return (output, e),
        };
    }

    (output, ExitCode::SUCCESS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_deploy_steps() {
        let mut steps = DeployStages::new();
        assert!(steps.0 == DeployStep::Initialize);
        steps.next();
        assert!(steps.0 == DeployStep::PreHook);
        steps.next();
        assert!(steps.0 == DeployStep::Symlink);
        steps.next();
        assert!(steps.0 == DeployStep::PostHook);
    }
}
