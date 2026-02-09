pub mod completer;
mod meta;

use std::{
    collections::HashMap,
    env,
    ffi::OsString,
    fmt::{self},
    fs::{self, OpenOptions},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use crate::command::meta::MetaSymbolExpander;

#[derive(Debug)]
pub enum Command {
    Exit,
    Echo(String),
    Type(Vec<Command>),
    Pwd,
    Cd(PathBuf),
    EnviromentalModifier {
        stdout_redirect: Option<RedirectInfo>,
        stderr_redirect: Option<RedirectInfo>,
    },
    External {
        exec_path: PathBuf,
        args: Vec<String>,
    },
    None(String),
}

#[derive(Debug, Clone)]
pub struct RedirectInfo {
    pub file_path: PathBuf,
    pub options: OpenOptions,
}

#[derive(Debug, PartialEq)]
enum CommandPartial {
    Exit,
    Echo,
    Type,
    Pwd,
    Cd,
    StdOutRedirect,
    StdOutRedirectAppend,
    StdErrRedirect,
    StdErrRedirectAppend,
    Unknown(String),
}

impl CommandPartial {
    fn parse(input: &str) -> CommandPartial {
        match input {
            ">" | "1>" => Self::StdOutRedirect,
            ">>" | "1>>" => Self::StdOutRedirectAppend,
            "2>" => Self::StdErrRedirect,
            "2>>" => Self::StdErrRedirectAppend,
            "exit" => Self::Exit,
            "echo" => Self::Echo,
            "type" => Self::Type,
            "pwd" => Self::Pwd,
            "cd" => Self::Cd,
            other => Self::Unknown(other.to_string()),
        }
    }

    fn can_be_chained_after(&self, other: &CommandPartial) -> bool {
        match other {
            _ => match self {
                Self::StdErrRedirect
                | Self::StdOutRedirect
                | Self::StdOutRedirectAppend
                | Self::StdErrRedirectAppend => true,
                _ => false,
            },
        }
    }

    fn into_full(&self, args: &Vec<String>) -> Command {
        match self {
            Self::Exit => Command::Exit,
            Self::Echo => Command::Echo(args.join(" ")),
            Self::Pwd => Command::Pwd,
            Self::Cd => Command::Cd(PathBuf::from(args.join(""))),
            Self::StdOutRedirect => {
                let mut options = OpenOptions::new();
                options.create(true).write(true).truncate(true);

                Command::EnviromentalModifier {
                    stdout_redirect: Some(RedirectInfo {
                        file_path: PathBuf::from(args.join("")),
                        options,
                    }),
                    stderr_redirect: None,
                }
            }
            Self::StdErrRedirect => {
                let mut options = OpenOptions::new();
                options.create(true).write(true).truncate(true);

                Command::EnviromentalModifier {
                    stdout_redirect: None,
                    stderr_redirect: Some(RedirectInfo {
                        file_path: PathBuf::from(args.join("")),
                        options,
                    }),
                }
            }
            Self::StdOutRedirectAppend => {
                let mut options = OpenOptions::new();
                options.create(true).append(true);

                Command::EnviromentalModifier {
                    stdout_redirect: Some(RedirectInfo {
                        file_path: PathBuf::from(args.join("")),
                        options,
                    }),
                    stderr_redirect: None,
                }
            }
            Self::StdErrRedirectAppend => {
                let mut options = OpenOptions::new();
                options.create(true).append(true);

                Command::EnviromentalModifier {
                    stdout_redirect: None,
                    stderr_redirect: Some(RedirectInfo {
                        file_path: PathBuf::from(args.join("")),
                        options,
                    }),
                }
            }
            Self::Type => {
                let inner_commands: Vec<Command> = args
                    .iter()
                    .flat_map(|arg| Command::parse(arg).commands)
                    .collect();
                Command::Type(inner_commands)
            }
            Self::Unknown(value) => {
                let exec_path = Command::get_executable_path(value);
                if let Some(path) = exec_path {
                    Command::External {
                        exec_path: path,
                        args: args.iter().map(|arg| String::from(arg)).collect(),
                    }
                } else {
                    Command::None(value.clone())
                }
            }
        }
    }
}

impl Command {
    pub fn parse(input: &str) -> CommandResult<'_> {
        let trimmed_input = input.trim();
        let tokens_iter = MetaSymbolExpander::new(trimmed_input.chars());

        let mut commands: Vec<Command> = Vec::with_capacity(10);

        let (mut current_partial, mut current_args) = (None::<CommandPartial>, Vec::new());

        let mut last_env_mod_index = 0;

        commands.push(Command::EnviromentalModifier {
            stderr_redirect: None,
            stdout_redirect: None,
        });

        for token in tokens_iter {
            if current_partial.is_none() {
                current_partial = Some(CommandPartial::parse(&token));
                continue;
            }

            let new_partial_cmd = CommandPartial::parse(&token);
            if new_partial_cmd.can_be_chained_after(current_partial.as_ref().unwrap()) {
                match current_partial.as_ref().unwrap() {
                    curr_partial @ CommandPartial::StdOutRedirect
                    | curr_partial @ CommandPartial::StdErrRedirect
                    | curr_partial @ CommandPartial::StdOutRedirectAppend
                    | curr_partial @ CommandPartial::StdErrRedirectAppend => {
                        if let Some(Command::EnviromentalModifier {
                            stdout_redirect,
                            stderr_redirect,
                        }) = commands.get_mut(last_env_mod_index)
                        {
                            let new_env_mod_cmd = curr_partial.into_full(&current_args);
                            if let Command::EnviromentalModifier {
                                stdout_redirect: new_stdout_redirect,
                                stderr_redirect: new_stderr_redirect,
                            } = new_env_mod_cmd
                            {
                                if new_stdout_redirect.is_some() {
                                    *stdout_redirect = new_stdout_redirect;
                                }
                                if new_stderr_redirect.is_some() {
                                    *stderr_redirect = new_stderr_redirect;
                                }
                            }
                        } else {
                            unreachable!(
                                "last_env_mod_index should point to Command::EnvironmentalModifier"
                            );
                        }
                        commands.push(Command::EnviromentalModifier {
                            stdout_redirect: None,
                            stderr_redirect: None,
                        });
                        last_env_mod_index = commands.len() - 1;
                    }
                    _ => {
                        commands.push(current_partial.unwrap().into_full(&current_args));
                    }
                }
                current_partial = Some(new_partial_cmd);
                current_args.clear();
            } else {
                current_args.push(token);
            }
        }

        if let Some(partial_cmd) = current_partial {
            match partial_cmd {
                CommandPartial::StdErrRedirect
                | CommandPartial::StdOutRedirect
                | CommandPartial::StdErrRedirectAppend
                | CommandPartial::StdOutRedirectAppend => {
                    if let Some(Command::EnviromentalModifier {
                        stdout_redirect,
                        stderr_redirect,
                    }) = commands.get_mut(last_env_mod_index)
                    {
                        if let Command::EnviromentalModifier {
                            stdout_redirect: new_stdout_redirect,
                            stderr_redirect: new_stderr_redirect,
                        } = partial_cmd.into_full(&current_args)
                        {
                            if new_stdout_redirect.is_some() {
                                *stdout_redirect = new_stdout_redirect;
                            }
                            if new_stderr_redirect.is_some() {
                                *stderr_redirect = new_stderr_redirect;
                            }
                        }
                    } else {
                        unreachable!(
                            "last_env_mod_index should point to Command::EnvironmentalModifier"
                        );
                    }
                }
                _ => {
                    commands.push(partial_cmd.into_full(&current_args));
                }
            }
        }

        CommandResult {
            input: trimmed_input,
            commands,
        }
    }

    fn get_executable_path(input: &str) -> Option<PathBuf> {
        let path = env::var_os("PATH").unwrap_or_default();
        for dir in env::split_paths(&path) {
            let exec_path = dir.join(input);
            if Command::is_executable(&exec_path) {
                return Some(exec_path);
            }
        }
        None
    }

    fn is_executable(path: &Path) -> bool {
        fs::metadata(path)
            .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
    }
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Command::Exit => write!(f, "exit"),
            Command::Pwd => write!(f, "pwd"),
            Command::Echo(_) => write!(f, "echo"),
            Command::Cd(_) => write!(f, "cd"),
            Command::Type(_) => write!(f, "type"),
            Command::EnviromentalModifier { .. } => {
                write!(f, "")
            }
            Command::External { exec_path, .. } => {
                write!(
                    f,
                    "{} is {}",
                    exec_path.file_name().unwrap_or_default().display(),
                    exec_path.display()
                )
            }
            Command::None(name) => write!(f, "{name}"),
        }
    }
}

pub fn get_external_commands(path: OsString) -> HashMap<OsString, PathBuf> {
    let mut executables = HashMap::new();

    for dir in env::split_paths(&path) {
        let dir_iter = fs::read_dir(&dir);
        if dir_iter.is_err() {
            continue;
        }
        for entry in fs::read_dir(&dir).unwrap() {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };

            let metadata = match entry.metadata() {
                Ok(meta) => meta,
                Err(_) => continue,
            };

            if metadata.is_file() {
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = metadata.permissions().mode();

                    // Any execute bit set (user, group, or other)
                    if mode & 0o111 != 0 {
                        executables.insert(entry.file_name(), entry.path());
                    }
                }
            }
        }
    }
    executables
}

#[derive(Debug)]
pub struct CommandResult<'a> {
    input: &'a str,
    pub commands: Vec<Command>,
}

#[cfg(test)]
mod test {}
