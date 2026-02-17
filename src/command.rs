pub mod completer;
mod meta;

use std::{
    collections::HashMap,
    env,
    ffi::OsString,
    fmt::{self},
    fs::{self, OpenOptions},
    io,
    path::PathBuf,
};

use crate::command::meta::MetaSymbolExpander;

pub const BUILTIN_COMMAND_NAMES: &[&str] = &[
    "exit", "echo", "type", "pwd", "cd", "history", ">", "1>", "2>", ">>", "1>>", "2>>", "|",
];

#[derive(Debug, Clone)]
pub enum StdOutRedirect {
    File {
        file_path: PathBuf,
        options: OpenOptions,
    },
    Pipe,
    None,
}

#[derive(Debug, Clone)]
pub enum StdErrRedirect {
    File {
        file_path: PathBuf,
        options: OpenOptions,
    },
    None,
}

#[derive(Debug, Clone)]
pub enum Command {
    Exit,
    Echo(String),
    Type(Vec<Command>),
    Pwd,
    Cd(PathBuf),
    History(HistoryParam),
    External {
        exec_path: PathBuf,
        args: Vec<String>,
    },
    None(String),
}

#[derive(Debug, Clone)]
pub enum HistoryParam {
    None,
    Limit(Option<usize>),
    ReadFromFile(PathBuf),
    WriteToFile(PathBuf),
    AppendToFile(PathBuf),
}

#[derive(Debug, PartialEq)]
enum PartialToken {
    Exit,
    Echo,
    Type,
    Pwd,
    Cd,
    History,
    StdOutRedirect,
    StdOutRedirectAppend,
    StdErrRedirect,
    StdErrRedirectAppend,
    Pipe,
    Unknown(String),
}

enum FinalToken {
    Command(Command),
    StdOutRedirect(StdOutRedirect),
    StdErrRedirect(StdErrRedirect),
}

impl PartialToken {
    fn parse(input: &str) -> PartialToken {
        match input {
            ">" | "1>" => Self::StdOutRedirect,
            ">>" | "1>>" => Self::StdOutRedirectAppend,
            "2>" => Self::StdErrRedirect,
            "2>>" => Self::StdErrRedirectAppend,
            "|" => Self::Pipe,
            "exit" => Self::Exit,
            "echo" => Self::Echo,
            "type" => Self::Type,
            "pwd" => Self::Pwd,
            "cd" => Self::Cd,
            "history" => Self::History,
            other => Self::Unknown(other.to_string()),
        }
    }

    fn can_be_chained_after(&self, other: &PartialToken) -> bool {
        match other {
            Self::Pipe => true,
            _ => match self {
                Self::StdErrRedirect
                | Self::StdOutRedirect
                | Self::StdOutRedirectAppend
                | Self::StdErrRedirectAppend
                | Self::Pipe => true,
                _ => false,
            },
        }
    }

    fn into_final(
        &self,
        args: &Vec<&str>,
        external_commands: &HashMap<OsString, PathBuf>,
    ) -> FinalToken {
        match self {
            Self::Exit => FinalToken::Command(Command::Exit),
            Self::Echo => FinalToken::Command(Command::Echo(args.join(" "))),
            Self::Pwd => FinalToken::Command(Command::Pwd),
            Self::Cd => FinalToken::Command(Command::Cd(PathBuf::from(args.join("")))),
            Self::History => {
                let mut args_iter = args.iter();
                let (first, second) = (args_iter.next(), args_iter.next());
                match (first, second) {
                    (None, None) | (None, Some(_)) => {
                        FinalToken::Command(Command::History(HistoryParam::None))
                    }
                    (Some(count), None) => {
                        let count = count.parse::<usize>().ok();
                        FinalToken::Command(Command::History(HistoryParam::Limit(count)))
                    }
                    (Some(arg), Some(file_path)) => {
                        if *arg == "-r" {
                            FinalToken::Command(Command::History(HistoryParam::ReadFromFile(
                                PathBuf::from(file_path),
                            )))
                        } else if *arg == "-w" {
                            FinalToken::Command(Command::History(HistoryParam::WriteToFile(
                                PathBuf::from(file_path),
                            )))
                        } else if *arg == "-a" {
                            FinalToken::Command(Command::History(HistoryParam::AppendToFile(
                                PathBuf::from(file_path),
                            )))
                        } else {
                            FinalToken::Command(Command::History(HistoryParam::None))
                        }
                    }
                }
            }
            Self::StdOutRedirect => {
                let mut options = OpenOptions::new();
                options.create(true).write(true).truncate(true);
                FinalToken::StdOutRedirect(StdOutRedirect::File {
                    file_path: PathBuf::from(args.join("")),
                    options,
                })
            }
            Self::StdErrRedirect => {
                let mut options = OpenOptions::new();
                options.create(true).write(true).truncate(true);
                FinalToken::StdErrRedirect(StdErrRedirect::File {
                    file_path: PathBuf::from(args.join("")),
                    options,
                })
            }
            Self::StdOutRedirectAppend => {
                let mut options = OpenOptions::new();
                options.create(true).append(true);
                FinalToken::StdOutRedirect(StdOutRedirect::File {
                    file_path: PathBuf::from(args.join("")),
                    options,
                })
            }
            Self::StdErrRedirectAppend => {
                let mut options = OpenOptions::new();
                options.create(true).append(true);

                FinalToken::StdErrRedirect(StdErrRedirect::File {
                    file_path: PathBuf::from(args.join("")),
                    options,
                })
            }
            Self::Pipe => FinalToken::StdOutRedirect(StdOutRedirect::Pipe),
            Self::Type => {
                let inner_commands: Vec<Command> = args
                    .iter()
                    .filter_map(|arg| Command::parse(arg, external_commands).ok())
                    .flat_map(|cr| cr.iter.into_commands())
                    .collect();
                FinalToken::Command(Command::Type(inner_commands))
            }
            Self::Unknown(value) => {
                let exec_path = external_commands.get(&OsString::from(value));
                if let Some(path) = exec_path {
                    FinalToken::Command(Command::External {
                        exec_path: path.clone(),
                        args: args.iter().map(|arg| String::from(*arg)).collect(),
                    })
                } else {
                    FinalToken::Command(Command::None(value.clone()))
                }
            }
        }
    }
}

impl Command {
    pub fn parse(
        input: &str,
        external_commands: &HashMap<OsString, PathBuf>,
    ) -> Result<CommandResult, io::Error> {
        let trimmed_input = input.trim();
        let tokens_iter = MetaSymbolExpander::new(trimmed_input.chars());

        let command_tokens: Vec<String> = tokens_iter.collect();

        let command_iter = Self::parse_from_tokens(&command_tokens, external_commands)?;

        Ok(CommandResult {
            iter: command_iter,
            metadata: CommandMetaData {
                input: input.to_string(),
                command_tokens,
            },
        })
    }

    pub fn parse_from_tokens(
        command_tokens: &[String],
        external_commands: &HashMap<OsString, PathBuf>,
    ) -> Result<CommandIter, io::Error> {
        let mut commands = Vec::with_capacity(command_tokens.len());
        let mut stdout_redirects = Vec::with_capacity(command_tokens.len());
        let mut stderr_redirects = Vec::with_capacity(command_tokens.len());

        let (mut current_partial, mut current_args): (Option<PartialToken>, Vec<&str>) =
            (None::<PartialToken>, Vec::new());

        for token in command_tokens {
            if current_partial.is_none() {
                current_partial = Some(PartialToken::parse(&token));
                continue;
            }

            let new_partial_cmd = PartialToken::parse(&token);
            if new_partial_cmd.can_be_chained_after(current_partial.as_ref().unwrap()) {
                let final_token = current_partial
                    .unwrap()
                    .into_final(&current_args, external_commands);
                match final_token {
                    FinalToken::Command(cmd) => {
                        commands.push(cmd);
                        stdout_redirects.push(StdOutRedirect::None);
                        stderr_redirects.push(StdErrRedirect::None);
                    }
                    FinalToken::StdOutRedirect(redirect) => {
                        if let Some(last_redirect) = stdout_redirects.last_mut() {
                            *last_redirect = redirect;
                        }
                    }
                    FinalToken::StdErrRedirect(redirect) => {
                        if let Some(last_redirect) = stderr_redirects.last_mut() {
                            *last_redirect = redirect;
                        }
                    }
                }
                current_partial = Some(new_partial_cmd);
                current_args.clear();
            } else {
                current_args.push(token);
            }
        }

        if let Some(partial_cmd) = current_partial {
            let final_token = partial_cmd.into_final(&current_args, external_commands);
            match final_token {
                FinalToken::Command(cmd) => {
                    commands.push(cmd);
                    stdout_redirects.push(StdOutRedirect::None);
                    stderr_redirects.push(StdErrRedirect::None);
                }
                FinalToken::StdOutRedirect(redirect) => {
                    if let Some(last_redirect) = stdout_redirects.last_mut() {
                        *last_redirect = redirect;
                    }
                }
                FinalToken::StdErrRedirect(redirect) => {
                    if let Some(last_redirect) = stderr_redirects.last_mut() {
                        *last_redirect = redirect;
                    }
                }
            }
        }

        Ok(CommandIter {
            stdout_redirects,
            stderr_redirects,
            commands,
        })
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
            Command::History(arg) => match arg {
                HistoryParam::Limit(count) => match count {
                    Some(limit) => write!(f, "history {limit}"),
                    None => write!(f, "history"),
                },
                HistoryParam::None => write!(f, "history"),
                HistoryParam::ReadFromFile(file_path) => {
                    write!(f, "history -r {}", file_path.display())
                }
                HistoryParam::WriteToFile(file_path) => {
                    write!(f, "history -w {}", file_path.display())
                }
                HistoryParam::AppendToFile(file_path) => {
                    write!(f, "history -a {}", file_path.display())
                }
            },
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
                        let dir_entry_path = entry.path();
                        if dir_entry_path.exists() {
                            let resolved_path =
                                fs::canonicalize(&dir_entry_path).unwrap_or(entry.path());
                            executables
                                .entry(entry.file_name())
                                .or_insert(resolved_path);
                        }
                    }
                }
            }
        }
    }
    executables
}

#[derive(Debug, Clone)]
pub struct CommandResult {
    pub metadata: CommandMetaData,
    pub iter: CommandIter,
}

#[derive(Debug, Clone)]
pub struct CommandMetaData {
    pub input: String,
    pub command_tokens: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct CommandIter {
    commands: Vec<Command>,
    stdout_redirects: Vec<StdOutRedirect>,
    stderr_redirects: Vec<StdErrRedirect>,
}

impl CommandIter {
    pub fn commands(&self) -> std::slice::Iter<'_, Command> {
        self.commands.iter()
    }

    pub fn commands_with_redirects(
        &self,
    ) -> impl Iterator<Item = (&Command, &StdOutRedirect, &StdErrRedirect)> {
        self.commands
            .iter()
            .zip(self.stdout_redirects.iter())
            .zip(self.stderr_redirects.iter())
            .map(|((cmd, stdout), stderr)| (cmd, stdout, stderr))
    }

    pub fn into_commands(self) -> Vec<Command> {
        self.commands
    }
}

#[cfg(test)]
mod test {}
