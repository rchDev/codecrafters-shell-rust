mod meta;

use std::{
    env, fmt, fs,
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
    External(PathBuf),
    None(String),
}

impl CommandPartial {
    fn parse(input: &str) -> CommandPartial {
        match input {
            "exit" => Self::Exit,
            "echo" => Self::Echo,
            "type" => Self::Type,
            "pwd" => Self::Pwd,
            "cd" => Self::Cd,
            other => match Command::get_executable_path(other) {
                Some(path) => Self::External(path),
                None => Self::None(other.to_string()),
            },
        }
    }

    fn can_be_chained_with(&self, _other: &CommandPartial) -> bool {
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
                let inner_commands: Vec<Command> =
                    args.iter().flat_map(|arg| Command::parse(arg)).collect();
                Command::Type(inner_commands)
            }
            Self::External(path) => Command::External {
                exec_path: path.clone(),
                args: args.into_iter().map(|s| s.to_string()).collect(),
            },
            Self::None(command_name) => Command::None(command_name.clone()),
        }
    }
}

impl Command {
    pub fn parse(input: &str) -> Vec<Command> {
        let trimmed_input = input.trim();
        let tokens_iter = MetaSymbolExpander::new(trimmed_input.chars());

        let mut commands: Vec<Command> = Vec::with_capacity(10);

        let (mut current_partial, mut current_args) = (None::<CommandPartial>, Vec::new());

        for token in tokens_iter {
            if current_partial.is_none() {
                current_partial = Some(CommandPartial::parse(&token));
                continue;
            }

            let partial_cmd = CommandPartial::parse(&token);
            if partial_cmd.can_be_chained_with(current_partial.as_ref().unwrap()) {
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

        commands
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

#[cfg(test)]
mod test {}
