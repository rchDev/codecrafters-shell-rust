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

impl Command {
    pub fn parse(input: &str) -> Command {
        let mut args = MetaSymbolExpander::new(input.chars());
        let (command, _) = (args.next().unwrap_or("".to_string()), args.next());

        match &command[..] {
            "exit" => Command::Exit,
            "echo" => {
                let args: Vec<String> = args.collect();
                Command::Echo(args.join(""))
            }
            "type" => {
                let inner_commands: Vec<Command> = args
                    .filter(|arg| !Command::str_contains_only_whitespace(arg.as_ref()))
                    .map(|arg| Command::parse(&arg))
                    .collect();
                Command::Type(inner_commands)
            }
            "pwd" => Command::Pwd,
            "cd" => {
                let args: Vec<String> = args
                    .filter(|arg| !Command::str_contains_only_whitespace(arg.as_ref()))
                    .collect();
                Command::Cd(PathBuf::from(args.join("")))
            }
            other => match Command::get_executable_path(other) {
                Some(exec_path) => {
                    let args: Vec<String> = args
                        .filter(|arg| !Command::str_contains_only_whitespace(arg.as_ref()))
                        .collect();
                    Command::External {
                        exec_path,
                        args: args,
                    }
                }
                None => Command::None(input.to_string()),
            },
        }
    }

    fn str_contains_only_whitespace(input: &str) -> bool {
        input.trim().is_empty()
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
