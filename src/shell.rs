pub use crate::command::Command;
pub use crate::command::completer::CommandCompleter;

use crate::command::{CommandResult, StdErrRedirect, StdOutRedirect};

use std::{
    collections::HashMap,
    env,
    fs::File,
    io::{self, Cursor, Read, Write},
    path::PathBuf,
    process::{self, Child, Command as StdProcCmd, Stdio},
};

pub struct CommandHistory {
    previous_user_input: Vec<String>,
    input_to_command_tokens: HashMap<String, Vec<String>>,
}

impl CommandHistory {
    fn new(history_size: usize) -> CommandHistory {
        CommandHistory {
            previous_user_input: Vec::with_capacity(history_size),
            input_to_command_tokens: HashMap::with_capacity(history_size),
        }
    }
}

pub struct Shell {
    working_dir: PathBuf,
    history: CommandHistory,
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
            history: CommandHistory::new(SHELL_DEFAULT_HISTORY_SIZE),
        }
    }

    pub fn set_history_size(size: usize) {
        unimplemented!();
    }

    fn add_history_entry(&mut self, cr: &CommandResult) -> Result<(), &'static str> {
        let input = cr.metadata.input.clone();
        self.history.previous_user_input.push(input.clone());

        let input_tokens = cr.metadata.command_tokens.clone();
        self.history
            .input_to_command_tokens
            .insert(input.clone(), input_tokens);

        Ok(())
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

    pub fn apply_commands(&mut self, command_result: Result<CommandResult, io::Error>) {
        let command_result = match command_result {
            Ok(result) => result,
            Err(error) => {
                let _ = writeln!(io::stderr(), "{}", error.to_string());
                return;
            }
        };

        match self.add_history_entry(&command_result) {
            Ok(()) => {}
            Err(_) => {
                panic!("failed to add history entry");
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
                    let execution_result = self.exec_builtin_command(builtin_command);
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

    fn exec_builtin_command(&mut self, command: &Command) -> Result<String, String> {
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
            Command::History(limit) => {
                const AVG_COMMAND_SIZE: usize = 20;
                let command_limit = if let Some(count) = *limit {
                    count
                } else {
                    self.history.previous_user_input.len()
                };

                let mut result = String::with_capacity(command_limit * AVG_COMMAND_SIZE);
                for (i, input) in self
                    .history
                    .previous_user_input
                    .iter()
                    .take(command_limit)
                    .enumerate()
                {
                    result += &format!("    {}  {input}\n", i + 1);
                }
                Ok(result)
            }
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
