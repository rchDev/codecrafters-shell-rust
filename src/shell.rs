pub use crate::command::Command;
pub use crate::command::completer::CommandCompleter;

use crate::command::{CommandResult, StdErrRedirect, StdOutRedirect};

use std::{
    cell::RefCell,
    env,
    io::{self, Cursor, Read, Write},
    path::PathBuf,
    process::{self, Child, Command as StdProcCmd, Stdio},
};

struct ExecutionContext {
    current_command_index: usize,
    last_command_index: usize,
    process_wait_stack: Vec<RefCell<Child>>,
    prev_out: Option<Box<dyn Read + Send>>,
}

pub struct Shell {
    working_dir: PathBuf,
}

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

    pub fn apply_commands(&mut self, command_result: Result<CommandResult, io::Error>) {
        let command_result = match command_result {
            Ok(result) => result,
            Err(error) => {
                writeln!(io::stderr(), "{}", error.to_string());
                return;
            }
        };

        let mut child_process_wait_list: Vec<Child> = Vec::with_capacity(4);

        let mut commands_iter = command_result.commands_with_redirects().peekable();
        let mut prev_out: Option<Box<dyn Read + Send>> = None;

        while let Some((command, out_redirect, err_redirect)) = commands_iter.next() {
            let is_last = &commands_iter.peek().is_none();
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
                                writeln!(io::stdout(), "{}", builtin_result);
                            }
                            StdOutRedirect::File { file_path, options } => {
                                match options.open(&file_path) {
                                    Ok(mut file_handle) => {
                                        writeln!(file_handle, "{}", builtin_result);
                                    }
                                    Err(_) => {
                                        writeln!(
                                            io::stdout(),
                                            "failed to open file for stdout redirection:\n{}",
                                            builtin_result
                                        );
                                    }
                                };
                            }
                        },
                        Err(error) => match err_redirect {
                            StdErrRedirect::File { file_path, options } => {
                                match options.open(&file_path) {
                                    Ok(mut file_handle) => {
                                        writeln!(file_handle, "{}", error);
                                    }
                                    Err(_) => {
                                        writeln!(
                                            io::stderr(),
                                            "failed to open file for stderr redirection:\n{}",
                                            error
                                        );
                                    }
                                }
                            }
                            StdErrRedirect::None => {
                                writeln!(io::stderr(), "{}", error);
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
                    writeln!(io::stderr(), "{}", err.to_string());
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
                    "cd: {}: No such file or directory",
                    exec_path.display()
                )),
            },
            Command::Echo(msg) => Ok(format!("{msg}")),
            Command::Type(inner_commands) => {
                let mut result = String::with_capacity(256);
                for (index, command) in inner_commands.iter().enumerate() {
                    if index > 0 {
                        result += "\n";
                    }
                    match command {
                        Command::None(name) => {
                            result += &format!("{name}: not found");
                        }
                        Command::External { exec_path, args: _ } => {
                            let res = format!(
                                "{} is {}",
                                exec_path.file_name().unwrap_or_default().display(),
                                exec_path.display()
                            );
                            result += &res;
                        }
                        builtin => result += &format!("{builtin} is a shell builtin"),
                    }
                }
                return Ok(result);
            }
            Command::Pwd => Ok(format!("{}", self.working_dir.display())),
            Command::Exit => {
                process::exit(0);
            }
            Command::None(cmd_name) => Err(format!("{cmd_name}: command not found")),
        }
    }

    fn change_dir(&mut self, path: PathBuf) {
        self.working_dir = path;
    }
}
