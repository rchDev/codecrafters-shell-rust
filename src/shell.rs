pub use crate::command::Command;
pub use crate::command::completer::CommandCompleter;

use crate::command::{CommandResult, RedirectInfo};

use std::{
    env,
    io::{self, Write},
    path::PathBuf,
    process::{self, Command as StdProcCmd, Stdio},
};
pub struct Shell {
    working_dir: PathBuf,
    stdout_redirect: Option<RedirectInfo>,
    stderr_redirect: Option<RedirectInfo>,
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
        }
    }

    pub fn apply_commands(&mut self, command_result: CommandResult) {
        for command in command_result.commands {
            self.exec_command(&command);
        }
    }

    fn exec_command(&mut self, command: &Command) {
        match &command {
            Command::Cd(exec_path) => {
                match env::set_current_dir(&exec_path) {
                    Ok(_) => self.change_dir(env::current_dir().unwrap()),
                    Err(_) => {
                        self.display_error(format!(
                            "cd: {}: No such file or directory",
                            exec_path.display()
                        ));
                    }
                };
            }
            Command::Echo(msg) => {
                self.display_result(format!("{msg}"));
            }

            Command::External { exec_path, args } => {
                let filename = exec_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();

                let mut cmd = StdProcCmd::new(filename);
                cmd.args(args).stdin(Stdio::inherit());

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
                        RedirectInfo::Pipe(inner_command) => {
                            todo!();
                            self.exec_command(&inner_command);
                        }
                    }
                } else {
                    cmd.stdout(Stdio::inherit());
                }

                if let Some(stderr_redirect) = self.stderr_redirect.clone() {
                    match stderr_redirect {
                        RedirectInfo::File { file_path, options } => {
                            match options.open(&file_path) {
                                Ok(file) => {
                                    cmd.stdout(Stdio::from(file));
                                }
                                Err(_) => {}
                            }
                        }
                        RedirectInfo::Pipe(inner_command) => {
                            todo!();
                            self.exec_command(&inner_command);
                        }
                    }
                } else {
                    cmd.stderr(Stdio::inherit());
                }

                let _ = cmd.status();
            }

            Command::Type(inner_commands) => {
                for command in inner_commands {
                    match command {
                        Command::None(name) => {
                            self.display_error(format!("{name}: not found"));
                        }
                        Command::External { exec_path, args: _ } => {
                            let res = format!(
                                "{} is {}",
                                exec_path.file_name().unwrap_or_default().display(),
                                exec_path.display()
                            );
                            self.display_result(res);
                        }
                        Command::EnviromentalModifier { .. } => {}
                        builtin => {
                            self.display_result(format!("{builtin} is a shell builtin"));
                        }
                    }
                }
            }

            Command::Pwd => {
                self.display_result(format!("{}", self.working_dir.display()));
            }

            Command::Exit => {
                process::exit(0);
            }

            Command::None(cmd_name) => {
                self.display_error(format!("{cmd_name}: command not found"));
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
                        RedirectInfo::Pipe(_) => {}
                    }
                }

                if let Some(stderr) = &self.stderr_redirect {
                    match stderr {
                        RedirectInfo::File { file_path, options } => {
                            _ = options.open(&file_path);
                        }
                        RedirectInfo::Pipe(_) => {}
                    }
                }
            }
        }
    }

    fn write_output<W: Write>(
        &self,
        text: String,
        redirect: &Option<RedirectInfo>,
        fallback_writer: &mut W,
    ) {
        if let Some(io_stream) = redirect {
            match io_stream {
                RedirectInfo::File { file_path, options } => match options.open(&file_path) {
                    Ok(mut file_handle) => _ = writeln!(file_handle, "{}", text),
                    Err(_) => {}
                },
                RedirectInfo::Pipe(inner_command) => {
                    dbg!(inner_command);
                    todo!();
                }
            }
        } else {
            _ = writeln!(fallback_writer, "{}", text);
        }
    }

    fn display_result(&self, text: String) {
        self.write_output(text, &self.stdout_redirect, &mut io::stdout());
    }

    fn display_error(&self, text: String) {
        self.write_output(text, &self.stderr_redirect, &mut io::stderr());
    }

    fn change_dir(&mut self, path: PathBuf) {
        self.working_dir = path;
    }
}
