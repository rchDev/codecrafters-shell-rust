pub use crate::command::Command;
pub use crate::command::completer::CommandCompleter;

use crate::command::{CommandResult, HistoryParam, StdErrRedirect, StdOutRedirect};

use rustyline::history::History;
use std::{
    borrow::Cow,
    env,
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Cursor, Read, Write},
    path::PathBuf,
    process::{self, Child, Command as StdProcCmd, Stdio},
};

pub struct CommandHistory {
    synced_index: usize,
    history_file_path: PathBuf,
    previous_user_input: Vec<String>,
}

impl CommandHistory {
    pub fn new(history_file_path: OsString) -> CommandHistory {
        CommandHistory {
            synced_index: 0,
            history_file_path: PathBuf::from(history_file_path),
            previous_user_input: Vec::with_capacity(SHELL_DEFAULT_HISTORY_SIZE),
        }
    }

    pub fn load_from_history_file(&mut self) {
        let file_path = self.history_file_path.clone();
        let _ = self.load(file_path.as_path());
    }
}

impl History for CommandHistory {
    fn add(&mut self, line: &str) -> rustyline::Result<bool> {
        self.previous_user_input.push(line.to_string());
        rustyline::Result::Ok(true)
    }
    fn add_owned(&mut self, line: String) -> rustyline::Result<bool> {
        self.previous_user_input.push(line);
        rustyline::Result::Ok(true)
    }

    fn append(&mut self, path: &std::path::Path) -> rustyline::Result<()> {
        let mut file_handle = OpenOptions::new().create(true).append(true).open(path)?;

        let in_mem_history_len = self.previous_user_input.len();
        let skip_amount = in_mem_history_len - (in_mem_history_len - self.synced_index);

        for entry in self.previous_user_input.iter().skip(skip_amount) {
            file_handle.write(format!("{}\n", entry).as_bytes())?;
        }
        self.synced_index = self.previous_user_input.len();

        rustyline::Result::Ok(())
    }

    fn clear(&mut self) -> rustyline::Result<()> {
        if let Some(last) = self.previous_user_input.pop() {
            self.previous_user_input.clear();
            self.previous_user_input.push(last);
            self.synced_index = 1;
        }
        rustyline::Result::Ok(())
    }
    fn get(
        &self,
        index: usize,
        _dir: rustyline::history::SearchDirection,
    ) -> rustyline::Result<Option<rustyline::history::SearchResult<'_>>> {
        let Some(entry) = self.previous_user_input.get(index) else {
            return rustyline::Result::Ok(None);
        };

        let search_result = rustyline::history::SearchResult {
            idx: index,
            pos: 0,
            entry: Cow::Borrowed(entry),
        };

        rustyline::Result::Ok(Some(search_result))
    }

    fn ignore_dups(&mut self, _yes: bool) -> rustyline::Result<()> {
        rustyline::Result::Ok(())
    }

    fn ignore_space(&mut self, _yes: bool) {}

    fn is_empty(&self) -> bool {
        self.previous_user_input.is_empty()
    }
    fn len(&self) -> usize {
        self.previous_user_input.len()
    }
    fn load(&mut self, path: &std::path::Path) -> rustyline::Result<()> {
        self.clear()?;
        let file_handle = OpenOptions::new().read(true).open(path)?;

        let reader = BufReader::new(file_handle);
        for line in reader.lines().filter_map(|line| line.ok()) {
            if !line.is_empty() {
                self.add_owned(line)?;
            }
        }
        self.synced_index = self.previous_user_input.len();

        rustyline::Result::Ok(())
    }

    fn save(&mut self, path: &std::path::Path) -> rustyline::Result<()> {
        let mut file_handle = OpenOptions::new().create(true).write(true).open(path)?;
        for entry in &self.previous_user_input {
            file_handle.write(format!("{}\n", entry).as_bytes())?;
        }

        rustyline::Result::Ok(())
    }

    fn search(
        &self,
        _term: &str,
        _start: usize,
        _dir: rustyline::history::SearchDirection,
    ) -> rustyline::Result<Option<rustyline::history::SearchResult<'_>>> {
        rustyline::Result::Ok(None)
    }
    fn set_max_len(&mut self, _len: usize) -> rustyline::Result<()> {
        unimplemented!()
    }
    fn starts_with(
        &self,
        _term: &str,
        _start: usize,
        _dir: rustyline::history::SearchDirection,
    ) -> rustyline::Result<Option<rustyline::history::SearchResult<'_>>> {
        unimplemented!()
    }
}

pub struct Shell {
    working_dir: PathBuf,
}

pub const SHELL_DEFAULT_HISTORY_SIZE: usize = 256;

impl Shell {
    /// Creates a new Shell object
    ///
    /// # Panics
    ///
    ///
    pub fn new() -> Self {
        Self {
            working_dir: env::current_dir().unwrap(),
        }
    }

    pub fn set_history_size(_size: usize) {
        unimplemented!();
    }

    fn open_redirect_files(
        stdout_redirect: &StdOutRedirect,
        stderr_redirect: &StdErrRedirect,
    ) -> (Option<File>, Option<File>) {
        let stdout_handle = match stdout_redirect {
            StdOutRedirect::File { file_path, options } => options.open(&file_path).ok(),
            _ => None,
        };

        let stderr_handle: Option<File> = match stderr_redirect {
            StdErrRedirect::File { file_path, options } => options.open(&file_path).ok(),
            _ => None,
        };

        (stdout_handle, stderr_handle)
    }

    pub fn apply_commands(
        &mut self,
        command_result: Result<CommandResult, io::Error>,
        history: &mut CommandHistory,
    ) {
        let command_result = match command_result {
            Ok(result) => result,
            Err(error) => {
                let _ = writeln!(io::stderr(), "{}", error.to_string());
                return;
            }
        };

        let mut child_process_wait_list: Vec<Child> = Vec::with_capacity(4);
        let mut commands_iter = command_result.iter.commands_with_redirects();
        let mut prev_out: Option<Box<dyn Read + Send>> = None;

        while let Some((command, out_redirect, err_redirect)) = commands_iter.next() {
            let (out_handle, err_handle) = Shell::open_redirect_files(out_redirect, err_redirect);
            match command {
                Command::External { exec_path, args } => {
                    let filename = exec_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default();

                    let mut cmd = StdProcCmd::new(filename);
                    cmd.args(args);

                    if prev_out.is_some() {
                        cmd.stdin(Stdio::piped());
                    } else {
                        cmd.stdin(Stdio::inherit());
                    }

                    match out_redirect {
                        StdOutRedirect::File { file_path, options } => {
                            match options.open(&file_path) {
                                Ok(file) => {
                                    cmd.stdout(Stdio::from(file));
                                }
                                Err(_) => {}
                            }
                        }
                        StdOutRedirect::Pipe => {
                            cmd.stdout(Stdio::piped());
                        }
                        StdOutRedirect::None => {
                            cmd.stdout(Stdio::inherit());
                        }
                    }

                    match err_redirect {
                        StdErrRedirect::File { file_path, options } => {
                            match options.open(&file_path) {
                                Ok(file) => {
                                    cmd.stderr(Stdio::from(file));
                                }
                                Err(_) => {}
                            }
                        }
                        StdErrRedirect::None => {
                            cmd.stderr(Stdio::inherit());
                        }
                    }

                    let mut child = match cmd.spawn() {
                        Ok(child) => child,
                        Err(_) => panic!(
                            "some child process could not be spawned for some unknown reason"
                        ),
                    };

                    if let Some(mut child_stdin) = child.stdin.take() {
                        if let Some(input) = prev_out.take() {
                            let _ = io::copy(&mut input.take(usize::MAX as u64), &mut child_stdin);
                        }
                        drop(child_stdin);
                    }

                    if let Some(child_stdout) = child.stdout.take() {
                        prev_out = Some(Box::new(child_stdout));
                    }

                    child_process_wait_list.push(child);
                }
                builtin_command => {
                    let execution_result = self.exec_builtin_command(builtin_command, history);
                    prev_out = None;
                    match execution_result {
                        Ok(builtin_result) => match out_redirect {
                            StdOutRedirect::Pipe => {
                                prev_out = Some(Box::new(Cursor::new(builtin_result)));
                            }
                            StdOutRedirect::None => {
                                if !builtin_result.is_empty() {
                                    let _ = write!(io::stdout(), "{}", builtin_result);
                                }
                            }
                            StdOutRedirect::File { .. } => match out_handle {
                                Some(mut handle) => {
                                    if !builtin_result.is_empty() {
                                        let _ = write!(handle, "{}", builtin_result);
                                    }
                                }
                                None => {
                                    if !builtin_result.is_empty() {
                                        let _ = write!(
                                            io::stderr(),
                                            "failed to open file for stdout redirection:\n{}",
                                            builtin_result
                                        );
                                    }
                                }
                            },
                        },
                        Err(error) => match err_redirect {
                            StdErrRedirect::File { .. } => match err_handle {
                                Some(mut handle) => {
                                    if !error.is_empty() {
                                        let _ = write!(handle, "{}", error);
                                    }
                                }
                                None => {
                                    if !error.is_empty() {
                                        let _ = write!(
                                            io::stderr(),
                                            "failed to open file for stderr redirection:\n{}",
                                            error
                                        );
                                    }
                                }
                            },
                            StdErrRedirect::None => {
                                if !error.is_empty() {
                                    let _ = write!(io::stderr(), "{}", error);
                                }
                            }
                        },
                    }
                }
            }
        }

        for child in child_process_wait_list.iter_mut().rev() {
            match child.wait() {
                Ok(_) => {}
                Err(err) => {
                    let _ = write!(io::stderr(), "{}", err.to_string());
                }
            };
        }
    }

    fn exec_builtin_command(
        &mut self,
        command: &Command,
        history: &mut CommandHistory,
    ) -> Result<String, String> {
        const AVG_COMMAND_SIZE: usize = 20;
        match command {
            Command::External { .. } => {
                unreachable!("EXETERNAL COMMAND REACH THE CODE PART IT SHOULDN'T HAVE REACHED");
            }
            Command::Cd(exec_path) => match env::set_current_dir(&exec_path) {
                Ok(_) => {
                    self.change_dir(env::current_dir().unwrap());
                    Ok(String::new())
                }
                Err(_) => Err(format!(
                    "cd: {}: No such file or directory\n",
                    exec_path.display()
                )),
            },
            Command::History(arg) => match arg {
                HistoryParam::None => {
                    let mut result = String::with_capacity(history.len() * AVG_COMMAND_SIZE);
                    for (i, input) in history.previous_user_input.iter().enumerate() {
                        result += &format!("    {}  {input}\n", i + 1);
                    }
                    Ok(result)
                }
                HistoryParam::Limit(count) => {
                    const AVG_COMMAND_SIZE: usize = 20;
                    let history_len = history.previous_user_input.len();
                    let elems_to_take = if let Some(count) = *count {
                        count
                    } else {
                        history_len
                    };

                    let mut result = String::with_capacity(elems_to_take * AVG_COMMAND_SIZE);
                    let elem_limit = history_len.saturating_sub(elems_to_take);
                    for (i, input) in history
                        .previous_user_input
                        .iter()
                        .enumerate()
                        .skip(elem_limit)
                    {
                        result += &format!("    {}  {input}\n", i + 1);
                    }
                    Ok(result)
                }
                HistoryParam::ReadFromFile(file_path) => {
                    let _ = history.load(file_path.as_path());
                    Ok("".to_string())
                }
                HistoryParam::WriteToFile(file_path) => {
                    let _ = history.save(file_path.as_path());
                    Ok("".to_string())
                }
                HistoryParam::AppendToFile(file_path) => {
                    let _ = history.append(file_path.as_path());
                    Ok("".to_string())
                }
            },
            Command::Echo(msg) => Ok(format!("{msg}\n")),
            Command::Type(inner_commands) => {
                let mut result = String::with_capacity(256);
                for command in inner_commands {
                    match command {
                        Command::None(name) => {
                            result += &format!("{name}: not found\n");
                        }
                        Command::External { exec_path, args: _ } => {
                            let res = format!(
                                "{} is {}\n",
                                exec_path.file_name().unwrap_or_default().display(),
                                exec_path.display()
                            );
                            result += &res;
                        }
                        builtin => result += &format!("{builtin} is a shell builtin\n"),
                    }
                }
                Ok(result)
            }
            Command::Pwd => Ok(format!("{}\n", self.working_dir.display())),
            Command::Exit => {
                process::exit(0);
            }
            Command::None(cmd_name) => Err(format!("{cmd_name}: command not found\n")),
        }
    }

    fn change_dir(&mut self, path: PathBuf) {
        self.working_dir = path;
    }
}
