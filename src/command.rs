mod meta;

use std::{
    env, fmt, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use meta::MetaChar;

use crate::command::meta::ExpansionBlocker;

pub enum Command {
    Exit,
    Echo(String),
    Type(Box<Command>),
    Pwd,
    Cd(PathBuf),
    External {
        exec_path: PathBuf,
        args: Vec<String>,
    },
    None(String),
}

impl Command {
    pub fn parse(s: &str) -> Command {
        let expanded_s = Command::expand_meta_chars(s);
        let mut args = expanded_s.trim().split(" ");
        let Some(cmd) = args.next() else {
            return Command::None("".to_string());
        };
        let args: Vec<&str> = args.collect();
        match cmd {
            "exit" => Command::Exit,
            "echo" => Command::Echo(args.join(" ")),
            "type" => {
                if args.len() == 0 || args.len() > 1 {
                    return Command::Type(Box::new(Command::None(args.join(" "))));
                }
                let inner_cmd = Command::parse(args[0]);
                Command::Type(Box::new(inner_cmd))
            }
            "pwd" => Command::Pwd,
            "cd" => Command::Cd(PathBuf::from(args.join(" "))),
            other => match Command::get_executable_path(other) {
                Some(exec_path) => Command::External {
                    exec_path,
                    args: args.into_iter().map(String::from).collect(),
                },
                None => Command::None(expanded_s.trim().to_string()),
            },
        }
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
            match (&expansion_blocker, new_expansion_blocker) {
                (None, None) => {}
                (Some(_), None) => {}
                (Some(blocker), Some(new_blocker)) if *blocker == new_blocker => {
                    expansion_blocker = None;
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
                    expansion_blocker = Some(new_blocker);
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
