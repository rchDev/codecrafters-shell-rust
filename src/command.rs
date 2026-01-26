mod meta;

use std::{
    borrow::Cow,
    env, fmt, fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use meta::MetaChar;

use crate::command::meta::Separator;

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
        let mut args = expanded_input.iter();
        let (command, _) = (args.next().unwrap().as_ref(), args.next());

        match command {
            "exit" => Command::Exit,
            "echo" => {
                let args: Vec<&str> = args.map(|arg| arg.as_ref()).collect();
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
                let args: Vec<&str> = args
                    .filter(|arg| !Command::str_contains_only_whitespace(arg.as_ref()))
                    .map(|arg| arg.as_ref())
                    .collect();
                Command::Cd(PathBuf::from(args.join("")))
            }
            other => match Command::get_executable_path(other) {
                Some(exec_path) => {
                    let args: Vec<String> = args
                        .filter(|arg| !Command::str_contains_only_whitespace(arg.as_ref()))
                        .map(|arg| arg.to_string())
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

    fn expand_meta_chars(input: &'_ str) -> Vec<Cow<'_, str>> {
        let mut current_meta: Option<MetaChar> = None;
        let mut out: Vec<Cow<str>> = Vec::with_capacity(5);
        let mut current_sep: Option<Separator> = None;
        let mut word_start: usize = 0;
        let mut temp_buffer: String = String::new();
        let mut prev_c: Option<char> = None;

        for (i, c) in input.char_indices() {
            let new_sep = Separator::try_from(c).ok();
            let mut sep_block_num = -1;
            let mut meta_block_num = -1;
            match (&current_sep, &new_sep) {
                (None, None) => {
                    sep_block_num = 1;
                }
                (None, Some(Separator::Whitespace(_))) => {
                    sep_block_num = 2;
                    let slice = &input[word_start..i];
                    if !slice.is_empty() {
                        if let Some(meta_char) = &current_meta {
                            let expanded_slice = meta_char.expand(slice);
                            out.push(Cow::Owned(expanded_slice));
                        } else {
                            out.push(Cow::Borrowed(slice));
                        }
                    }
                    if let Some(prev_c_val) = prev_c {
                        match Separator::try_from(prev_c_val) {
                            Ok(Separator::Whitespace(_)) => {}
                            _ => {
                                out.push(Cow::Borrowed(" "));
                            }
                        }
                    }
                    current_meta = None;
                    word_start = i + 1;
                }
                (None, Some(_)) => {
                    sep_block_num = 3;
                    let slice = &input[word_start..i];
                    if !slice.is_empty() {
                        if let Some(meta_char) = &current_meta {
                            let expanded_slice = meta_char.expand(slice);
                            out.push(Cow::Owned(expanded_slice));
                        } else {
                            out.push(Cow::Borrowed(slice));
                        }
                    }
                    current_meta = None;
                    current_sep = new_sep;
                    word_start = i + 1;
                }
                (Some(old_sep_value), Some(new_sep_value)) if *old_sep_value == *new_sep_value => {
                    sep_block_num = 4;
                    let slice = &input[word_start..i];
                    if let Some(meta_char) = &current_meta {
                        let expanded_char = meta_char.expand(slice);
                        temp_buffer.push_str(&expanded_char);
                    } else {
                        temp_buffer.push_str(slice);
                    }
                    out.push(Cow::Owned(temp_buffer.clone()));

                    temp_buffer.clear();
                    current_sep = None;
                    current_meta = None;
                    word_start = i + 1;
                }
                (Some(Separator::Whitespace(_)), Some(Separator::Whitespace(_))) => {
                    sep_block_num = 5;
                }
                (Some(curr_sep), Some(Separator::Whitespace(_)))
                    if current_meta
                        .as_ref()
                        .is_some_and(|cm| curr_sep.allows_meta_char(&cm)) =>
                {
                    sep_block_num = 6;
                    let slice = &input[word_start..i];
                    let expanded_slice = current_meta.unwrap().expand(slice);
                    out.push(Cow::Owned(expanded_slice));
                    word_start = i + 1;
                    current_meta = None;
                    out.push(Cow::Borrowed(" "));
                }
                (Some(_), Some(_)) | (Some(_), None) => {
                    sep_block_num = 7;
                }
            }

            let new_meta = MetaChar::try_from(c).ok();
            dbg!(&new_meta, sep_block_num);
            match (&current_meta, &new_meta, &current_sep) {
                (None, None, _) => {
                    meta_block_num = 1;
                }
                (None, Some(_), None) => {
                    meta_block_num = 2;
                    let slice = &input[word_start..i];
                    if !slice.is_empty() {
                        out.push(Cow::Borrowed(slice));
                    }
                    word_start = i + 1;
                    current_meta = new_meta;
                }
                (None, Some(new_val), Some(curr_sep)) if curr_sep.allows_meta_char(new_val) => {
                    meta_block_num = 3;
                    current_meta = new_meta;
                }
                (Some(old_val), Some(_), None) => {
                    meta_block_num = 4;
                    let slice = &input[word_start..i];
                    if !slice.is_empty() {
                        let expanded_slice = old_val.expand(slice);
                        out.push(Cow::Owned(expanded_slice));
                    }
                    word_start = i + 1;
                    current_meta = new_meta;
                }
                (Some(old_val), Some(new_val), Some(curr_sep))
                    if curr_sep.allows_meta_char(new_val) =>
                {
                    meta_block_num = 5;
                    let slice = &input[word_start..i];
                    if !slice.is_empty() {
                        let expanded_slice = old_val.expand(slice);
                        temp_buffer.push_str(&expanded_slice);
                    }
                    word_start = i + 1;
                    current_meta = new_meta;
                }
                (_, _, _) => {
                    meta_block_num = 6;
                }
            }
            prev_c = Some(c);
            dbg!(meta_block_num);
        }
        let slice = &input[word_start..];
        if !slice.is_empty() {
            if let Some(meta_val) = &current_meta {
                let expanded = meta_val.expand(slice);
                out.push(Cow::Owned(expanded));
            } else {
                out.push(Cow::Borrowed(slice));
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

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn expand_meta_chars_works_base() {
        let test_input = String::from("asdas$HOME asdasda\n");
        let result = Command::expand_meta_chars(&test_input);
        dbg!(result);

        let test_input = String::from("asdas \"asds   ada\"  $HOME asdasda\n");
        let result = Command::expand_meta_chars(&test_input);
        dbg!(result);
        let test_input = String::from("\"world  shell\"  \"example\"\"script\"");
        let result = Command::expand_meta_chars(&test_input);
        dbg!(result);
    }
}
