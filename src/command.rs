mod meta;

use std::{
    env, fmt, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use meta::MetaChar;

use crate::command::meta::ExpansionBlocker;

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
        let expanded_input = Command::expand_meta_chars(input);
        let expanded_trimmed_input = expanded_input.trim();

        let (command, args) = match expanded_trimmed_input.split_once(char::is_whitespace) {
            Some((cmd, rest)) => (cmd, rest),
            None => (expanded_trimmed_input, ""),
        };

        // let args = Command::split_args(args);
        match command {
            "exit" => Command::Exit,
            "echo" => Command::Echo(args.to_string()),
            "type" => {
                let inner_commands: Vec<Command> = Command::split_args(args)
                    .iter()
                    .map(|arg| {
                        let clean_arg = Command::strip_outer_quotes(arg);
                        Command::parse(&clean_arg)
                    })
                    .collect();
                Command::Type(inner_commands)
            }
            "pwd" => Command::Pwd,
            "cd" => Command::Cd(PathBuf::from(args)),
            other => match Command::get_executable_path(other) {
                Some(exec_path) => Command::External {
                    exec_path,
                    args: Command::split_args(args)
                        .into_iter()
                        .map(String::from)
                        .collect(),
                },
                None => Command::None(expanded_trimmed_input.to_string()),
            },
        }
    }

    fn strip_outer_quotes(input: &str) -> &str {
        input
            .strip_prefix('\'')
            .and_then(|stripped| stripped.strip_suffix('\''))
            .or_else(|| {
                input
                    .strip_prefix('"')
                    .and_then(|stripped| stripped.strip_suffix('"'))
            })
            .unwrap_or(input)
    }

    fn split_args(input: &str) -> Vec<&str> {
        if input.is_empty() {
            return vec![input];
        }
        let mut output: Vec<&str> = Vec::new();
        let mut current_split: Option<char> = None;
        let mut word_start: usize = 0;

        for (i, c) in input.char_indices() {
            if let Some(split_char) = current_split
                && split_char == c
            {
                let slice = &input[word_start..=i];
                if !slice.is_empty() {
                    output.push(slice);
                }
                current_split = None;
                word_start = i + 1;
            } else if current_split.is_none() && c.is_whitespace() {
                let slice = &input[word_start..i];
                if !slice.is_empty() {
                    output.push(slice)
                }
                word_start = i + 1;
            } else if current_split.is_none() && (c == '\'' || c == '"') {
                current_split = Some(c);
                let slice = &input[word_start..i];
                if !slice.is_empty() {
                    output.push(slice);
                }
                word_start = i;
            }
        }

        if word_start < input.len() - 1 {
            let slice = &input[word_start..];
            if !slice.is_empty() {
                output.push(slice);
            }
        }
        if output.is_empty() {
            output.push(&input[..])
        }

        output
    }

    fn expand_meta_chars(s: &str) -> String {
        let mut current_meta: Option<MetaChar> = None;
        let mut out = String::with_capacity(s.len());
        let mut expansion_buf = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        let mut expansion_blocker: Option<ExpansionBlocker> = None;

        while let Some(c) = chars.next() {
            // set expansion blocker
            let new_expansion_blocker = ExpansionBlocker::try_from(c).ok();
            match (&expansion_blocker, &new_expansion_blocker) {
                (None, None) => {}
                (Some(_), None) => {}
                (Some(blocker), Some(new_blocker)) if *blocker == *new_blocker => {
                    expansion_blocker = None;

                    if let Some(meta_char) = &current_meta {
                        meta_char.expand(&mut expansion_buf, &mut out);
                        current_meta = None;
                    }
                    out.push(new_blocker.name());
                    continue;
                }
                (Some(_), Some(_)) => {
                    out.push(c);
                }
                (None, Some(new_blocker)) => {
                    if let Some(meta_char) = &current_meta
                        && !new_blocker.allows_meta_char(&meta_char)
                    {
                        match meta_char {
                            MetaChar::Whitespace(_) => out.push(' '),
                            _ => {}
                        }
                        current_meta = None;
                    }
                    expansion_blocker = Some(*new_blocker);
                    out.push(new_blocker.name());
                    continue;
                }
            }

            let new_char = MetaChar::try_from(c).ok();
            match (&current_meta, new_char, &expansion_blocker) {
                (None, None, _) => {
                    out.push(c);
                }
                (None, Some(meta_char), None) => {
                    current_meta = Some(meta_char);
                }
                (_, Some(new), Some(expansion_blocker)) => {
                    if expansion_blocker.allows_meta_char(&new) {
                        current_meta = Some(new);
                    } else {
                        out.push(c);
                    }
                }
                (Some(MetaChar::Whitespace(_)), Some(MetaChar::Whitespace(_)), _) => {}
                (Some(prev), ws_char @ Some(MetaChar::Whitespace(_)), _) => {
                    prev.expand(&mut expansion_buf, &mut out);
                    current_meta = ws_char;
                }
                (Some(MetaChar::Whitespace(_)), new_meta @ Some(_), _) => {
                    out.push(' ');
                    current_meta = new_meta;
                }
                (Some(MetaChar::Whitespace(_)), None, _) => {
                    out.push(' ');
                    out.push(c);
                    current_meta = None;
                }
                (Some(_), _, _) => {
                    expansion_buf.push(c);
                }
            }
        }
        out
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
