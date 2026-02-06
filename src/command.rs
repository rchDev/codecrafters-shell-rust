mod meta;

use std::{
    env,
    fmt::{self},
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use crate::command::meta::MetaSymbolExpander;

const STDOUT_REDIRECT_TO_FILE_V1: &str = ">";
const STDOUT_REDIRECT_TO_FILE_V2: &'static str = "1>";
const STDERR_REDIRECT_TO_FILE: &str = "2>";

#[derive(Debug)]
pub enum Command {
    Exit,
    Echo(String),
    Type(Vec<Command>),
    Pwd,
    Cd(PathBuf),
    EnviromentalModifier {
        stdout_redirect: Option<PathBuf>,
        stderr_redirect: Option<PathBuf>,
    },
    External {
        exec_path: PathBuf,
        args: Vec<String>,
    },
    None(String),
}

#[derive(Debug, PartialEq)]
enum CommandPartial {
    Exit,
    Echo,
    Type,
    Pwd,
    Cd,
    Unknown(String),
}

impl CommandPartial {
    fn parse(input: &str) -> CommandPartial {
        match input {
            "exit" => Self::Exit,
            "echo" => Self::Echo,
            "type" => Self::Type,
            "pwd" => Self::Pwd,
            "cd" => Self::Cd,
            other => Self::Unknown(other.to_string()),
        }
    }

    fn can_be_chained_after(&self, _other: &CommandPartial) -> bool {
        match self {
            _ => false,
        }
    }

    fn into_full(&self, args: &Vec<String>) -> Command {
        match self {
            Self::Exit => Command::Exit,
            Self::Echo => Command::Echo(args.join(" ")),
            Self::Pwd => Command::Pwd,
            Self::Cd => Command::Cd(PathBuf::from(args.join(""))),
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

        let last_env_mod_index = 0;

        commands.push(Command::EnviromentalModifier {
            stderr_redirect: None,
            stdout_redirect: None,
        });

        for (i, token) in tokens_iter.enumerate() {
            if current_partial.is_none() {
                current_partial = Some(CommandPartial::parse(&token));
                continue;
            }

            let partial_cmd = CommandPartial::parse(&token);

            if partial_cmd.can_be_chained_after(current_partial.as_ref().unwrap()) {
                commands.push(current_partial.unwrap().into_full(&current_args));
                current_partial = None;
                current_args.clear();
            } else {
                current_args.push(token);
            }
        }

        if let Some(partial_cmd) = current_partial {
            commands.push(partial_cmd.into_full(&current_args));
        }

        CommandResult { input, commands }
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

pub struct CommandResult<'a> {
    input: &'a str,
    pub commands: Vec<Command>,
}

#[cfg(test)]
mod test {}
