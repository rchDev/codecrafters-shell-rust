pub use crate::command::Command;
use crate::command::CommandResult;

use std::{
    env,
    fs::File,
    io::{self, Stdout, Write},
    path::PathBuf,
    process::{self, Command as StdProcCmd, Stdio},
};
pub struct Shell {
    working_dir: PathBuf,
    stdout_redirect_path: Option<PathBuf>,
    stderr_redirect_path: Option<PathBuf>,
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
            stdout_redirect_path: None,
            stderr_redirect_path: None,
        }
    }

    pub fn exec_command(&mut self, command_result: CommandResult) {
        for cmd in command_result.commands {
            match &cmd {
                Command::Cd(exec_path) => {
                    match env::set_current_dir(&exec_path) {
                        Ok(_) => self.change_dir(env::current_dir().unwrap()),
                        Err(_) => {
                            self.display_result(
                                format!("cd: {}: No such file or directory", exec_path.display()),
                                io::stderr(),
                            );
                        }
                    };
                }
                Command::Echo(msg) => {
                    self.display_result(format!("{msg}"), io::stdout());
                }

                Command::External { exec_path, args } => {
                    let filename = exec_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default();

                    let output = StdProcCmd::new(filename)
                        .args(args)
                        .stdin(Stdio::inherit())
                        .stdout(Stdio::inherit())
                        .stderr(Stdio::inherit())
                        .output();
                }

                Command::Type(inner_commands) => {
                    for command in inner_commands {
                        match command {
                            Command::None(name) => {
                                self.display_result(format!("{name}: not found"), io::stderr());
                            }
                            Command::External { exec_path, args: _ } => {
                                let res = format!(
                                    "{} is {}",
                                    exec_path.file_name().unwrap_or_default().display(),
                                    exec_path.display()
                                );
                                self.display_result(res, io::stdout());
                            }
                            builtin => {
                                self.display_result(
                                    format!("{builtin} is a shell builtin"),
                                    io::stdout(),
                                );
                            }
                        }
                    }
                }

                Command::Pwd => {
                    self.display_result(format!("{}", self.working_dir.display()), io::stdout());
                }

                Command::Exit => {
                    process::exit(0);
                }

                Command::None(cmd_name) => {
                    self.display_result(format!("{cmd_name}: command not found"), io::stderr());
                }

                Command::EnviromentalModifier {
                    stdout_redirect,
                    stderr_redirect,
                } => {
                    self.stdout_redirect_path = stdout_redirect.clone();
                    self.stderr_redirect_path = stderr_redirect.clone();
                }
            }
        }
    }

    fn display_result<W: Write>(&self, text: String, mut out: W) {
        if let Some(redirect_path) = &self.stdout_redirect_path {
            match File::create(redirect_path) {
                Ok(mut file_handle) => {
                    _ = writeln!(file_handle, "{}", text);
                }
                Err(_) => {}
            }
        } else {
            _ = writeln!(out, "{}", text);
        }
    }

    fn change_dir(&mut self, path: PathBuf) {
        self.working_dir = path;
    }
}
