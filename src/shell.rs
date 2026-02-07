pub use crate::command::Command;
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

    pub fn exec_command(&mut self, command_result: CommandResult) {
        for cmd in command_result.commands {
            match &cmd {
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

                    if let Some(stdout_redirect) = &self.stdout_redirect {
                        match stdout_redirect.options.open(&stdout_redirect.file_path) {
                            Ok(file) => {
                                cmd.stdout(Stdio::from(file));
                            }
                            Err(_) => {}
                        }
                    } else {
                        cmd.stdout(Stdio::inherit());
                    }

                    if let Some(stderr_redirect) = &self.stderr_redirect {
                        match stderr_redirect.options.open(&stderr_redirect.file_path) {
                            Ok(file) => {
                                cmd.stderr(Stdio::from(file));
                            }
                            Err(_) => {}
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
                        _ = stdout.options.open(&stdout.file_path);
                    }

                    if let Some(stderr) = &self.stderr_redirect {
                        _ = stderr.options.open(&stderr.file_path);
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
            match io_stream.options.open(&io_stream.file_path) {
                Ok(mut file_handle) => {
                    _ = writeln!(file_handle, "{}", text);
                }
                Err(_) => {}
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
