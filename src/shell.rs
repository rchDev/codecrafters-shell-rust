pub use crate::command::Command;
pub use crate::command::completer::CommandCompleter;

use crate::command::CommandResult;

use std::{
    cell::RefCell,
    env,
    io::{self, Read, Write},
    path::PathBuf,
    process::{self, Child, ChildStdout, Command as StdProcCmd, Stdio},
};

struct ExecutionContext {
    current_command_index: usize,
    last_command_index: usize,
    process_wait_stack: Vec<RefCell<Child>>,
    prev_out: Option<Box<dyn Read + Send>>,
}

impl ExecutionContext {
    fn new() -> ExecutionContext {
        Self {
            current_command_index: 0,
            last_command_index: 0,
            process_wait_stack: Vec::with_capacity(4),
            prev_out: None,
        }
    }
}
pub struct Shell {
    working_dir: PathBuf,
    stdout_redirect: Option<RedirectInfo>,
    stderr_redirect: Option<RedirectInfo>,
    execution_context: ExecutionContext,
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
            stdout_redirect: None,
            stderr_redirect: None,
            execution_context: ExecutionContext::new(),
        }
    }

    pub fn apply_commands(&mut self, command_result: CommandResult) {
        if command_result.commands.is_empty() {
            return;
        }

        self.execution_context.last_command_index = command_result.commands.len() - 1;

        for command in command_result.commands {
            self.exec_command(&command);
            self.execution_context.current_command_index += 1;
        }

        self.execution_context.current_command_index = 0;
    }

    fn current_command_is_last(&self) -> bool {
        return self.execution_context.current_command_index
            == self.execution_context.last_command_index;
    }

    fn exec_command(&mut self, command: &Command) {
        match &command {
            Command::Cd(exec_path) => {
                self.execution_context.prev_out = None;
                match env::set_current_dir(&exec_path) {
                    Ok(_) => self.change_dir(env::current_dir().unwrap()),
                    Err(_) => {
                        self.write_error(format!(
                            "cd: {}: No such file or directory",
                            exec_path.display()
                        ));
                    }
                };
            }
            Command::Echo(msg) => {
                self.execution_context.prev_out = None;
                self.write_result(format!("{msg}"));
            }

            Command::External { exec_path, args } => {
                let filename = exec_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();

                let mut cmd = StdProcCmd::new(filename);
                cmd.args(args);

                // set stdin
                if let Some(prev_out) = &mut self.execution_context.prev_child_stdout {
                    cmd.stdin(Stdio::from(*prev_out.borrow()));
                } else if !self.execution_context.stdout_buffer.is_empty() {
                    cmd.stdin(Stdio::piped());
                } else {
                    cmd.stdin(Stdio::inherit());
                }

                // set stdout
                if let Some(stdout_redirect) = self.stdout_redirect.clone() {
                    match stdout_redirect {
                        RedirectInfo::File { file_path, options } => {
                            match options.open(&file_path) {
                                Ok(file) => {
                                    cmd.stdout(Stdio::from(file));
                                }
                                Err(_) => {}
                            }
                        }
                        RedirectInfo::Pipe => {
                            cmd.stdout(Stdio::piped());
                        }
                    }
                } else {
                    cmd.stdout(Stdio::inherit());
                }

                // set stderr
                if let Some(stderr_redirect) = self.stderr_redirect.clone() {
                    match stderr_redirect {
                        RedirectInfo::File { file_path, options } => {
                            match options.open(&file_path) {
                                Ok(file) => {
                                    cmd.stderr(Stdio::from(file));
                                }
                                Err(_) => {}
                            }
                        }
                        RedirectInfo::Pipe => {
                            // do nothing
                        }
                    }
                } else {
                    cmd.stderr(Stdio::inherit());
                }

                // spawn the process
                let mut child_process = match cmd.spawn() {
                    Ok(child_process) => child_process,
                    Err(e) => {
                        self.write_error(e.to_string());
                        return;
                    }
                };

                // if child processes' stdin is piped
                if let Some(child_stdin) = &mut child_process.stdin {
                    child_stdin.write(&self.execution_context.stdout_buffer);
                }

                // if child processes' stdout is piped
                //
                if let Some(child_stdout) = child_process.stdout {
                    self.execution_context.prev_child_stdout = Some(RefCell::new(child_stdout));
                }

                // if let Some(mut stdout) = child_process.stdout.take() {
                //     dbg!("IM HERE stdout.take");
                //     match stdout.read_to_end(&mut self.stdout_buffer) {
                //         Ok(_) => {}
                //         Err(e) => self.write_error(e.to_string()),
                //     }
                //     drop(stdout);
                // }

                dbg!("taking stdout");
                let child_processes_output =
                    child_process.stdout.take().expect("c1 stdout missing");

                dbg!("waiting for child_process");
                let _ = child_process.wait();

                dbg!("writing child_process results");
                let child_stdout: Vec<u8> = child_processes_output
                    .bytes()
                    .filter_map(|byte| byte.ok())
                    .collect();
            }

            Command::Type(inner_commands) => {
                self.execution_context.stdout_buffer.clear();
                self.execution_context.prev_child_stdout = None;
                for command in inner_commands {
                    match command {
                        Command::None(name) => {
                            self.write_error(format!("{name}: not found"));
                        }
                        Command::External { exec_path, args: _ } => {
                            let res = format!(
                                "{} is {}",
                                exec_path.file_name().unwrap_or_default().display(),
                                exec_path.display()
                            );
                            self.write_result(res);
                        }
                        Command::EnviromentalModifier { .. } => {}
                        builtin => {
                            self.write_result(format!("{builtin} is a shell builtin"));
                        }
                    }
                }
            }

            Command::Pwd => {
                self.execution_context.stdout_buffer.clear();
                self.execution_context.prev_child_stdout = None;
                self.write_result(format!("{}", self.working_dir.display()));
            }

            Command::Exit => {
                process::exit(0);
            }

            Command::None(cmd_name) => {
                self.execution_context.stdout_buffer.clear();
                self.execution_context.prev_child_stdout = None;
                self.write_error(format!("{cmd_name}: command not found"));
            }

            Command::EnviromentalModifier {
                stdout_redirect,
                stderr_redirect,
            } => {
                self.stdout_redirect = stdout_redirect.clone();
                self.stderr_redirect = stderr_redirect.clone();

                if let Some(stdout) = &self.stdout_redirect {
                    match stdout {
                        RedirectInfo::File { file_path, options } => {
                            _ = options.open(&file_path);
                        }
                        RedirectInfo::Pipe => {
                            // do nothing
                        }
                    }
                }

                if let Some(stderr) = &self.stderr_redirect {
                    match stderr {
                        RedirectInfo::File { file_path, options } => {
                            _ = options.open(&file_path);
                        }
                        RedirectInfo::Pipe => {
                            // do_nothing
                        }
                    }
                }
            }
        }
    }

    fn write_output<W: Write>(
        &mut self,
        output: String,
        redirect: &Option<RedirectInfo>,
        fallback_writer: &mut W,
    ) {
        if let Some(io_stream) = redirect {
            match io_stream {
                RedirectInfo::File { file_path, options } => match options.open(&file_path) {
                    Ok(mut file_handle) => _ = writeln!(file_handle, "{}", output),
                    Err(_) => {}
                },
                RedirectInfo::Pipe => {
                    self.execution_context
                        .stdout_buffer
                        .extend_from_slice(output.as_bytes());
                }
            }
        } else {
            _ = writeln!(fallback_writer, "{}", output);
        }
    }

    fn write_result(&mut self, text: String) {
        self.write_output(text, &self.stdout_redirect.clone(), &mut io::stdout());
    }

    fn write_error(&mut self, text: String) {
        self.write_output(text, &self.stderr_redirect.clone(), &mut io::stderr());
    }

    fn change_dir(&mut self, path: PathBuf) {
        self.working_dir = path;
    }
}
