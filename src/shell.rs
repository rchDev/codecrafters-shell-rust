pub use crate::command::Command;

use std::{
    env,
    path::PathBuf,
    process,
    process::{Command as StdProcCmd, Stdio},
};

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

    pub fn exec_command(&mut self, commands: Vec<Command>) {
        let mut cmd_out_for_display: Vec<Result<String, String>> =
            Vec::with_capacity(commands.len());

        for cmd in commands {
            match &cmd {
                Command::Cd(exec_path) => {
                    match env::set_current_dir(&exec_path) {
                        Ok(_) => self.change_dir(env::current_dir().unwrap()),
                        Err(_) => {
                            cmd_out_for_display.push(Err(format!(
                                "cd: {}: No such file or directory",
                                exec_path.display()
                            )));
                        }
                    };
                }
                Command::Echo(msg) => {
                    cmd_out_for_display.push(Ok(format!("{}", msg)));
                }

                Command::StdoutRedirect(file_path) => {}

                Command::External { exec_path, args } => {
                    let filename = exec_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default();

                    let _ = StdProcCmd::new(filename)
                        .args(args)
                        .stdin(Stdio::inherit())
                        .stdout(Stdio::inherit())
                        .stderr(Stdio::inherit())
                        .status();
                }
                Command::Type(inner_commands) => {
                    for command in inner_commands {
                        match command {
                            Command::None(name) => {
                                cmd_out_for_display.push(Err(format!("{name}: not found")));
                            }
                            Command::External { exec_path, args: _ } => {
                                let res = format!(
                                    "{} is {}",
                                    exec_path.file_name().unwrap_or_default().display(),
                                    exec_path.display()
                                );
                                cmd_out_for_display.push(Ok(res));
                            }
                            builtin => {
                                cmd_out_for_display
                                    .push(Ok(format!("{builtin} is a shell builtin")));
                            }
                        }
                    }
                }
                Command::Pwd => {
                    cmd_out_for_display.push(Ok(format!("{}", self.working_dir.display())));
                }
                Command::Exit => {
                    process::exit(0);
                }
                Command::None(cmd_name) => {
                    cmd_out_for_display.push(Err(format!("{cmd_name}: command not found")));
                }
            }
        }

        for result in command_results {
            match result {
                Ok(res_text) => println!("{res_text}"),
                Err(err_text) => eprintln!("{err_text}"),
            }
        }
    }

    fn change_dir(&mut self, path: PathBuf) {
        self.working_dir = path;
    }
}
