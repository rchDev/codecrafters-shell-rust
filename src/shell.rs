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

    pub fn exec_command(&mut self, cmd: Command) {
        match cmd {
            Command::Cd(exec_path) => {
                match env::set_current_dir(&exec_path) {
                    Ok(_) => self.change_dir(env::current_dir().unwrap()),
                    Err(_) => {
                        eprintln!("cd: {}: No such file or directory", exec_path.display())
                    }
                };
            }
            Command::Echo(msg) => {
                println!("{}", msg.trim());
            }
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
                        Command::None(name) => println!("{name}: not found"),
                        Command::External { exec_path, args: _ } => println!(
                            "{} is {}",
                            exec_path.file_name().unwrap_or_default().display(),
                            exec_path.display()
                        ),
                        builtin => println!("{builtin} is a shell builtin"),
                    }
                }
            }
            Command::Pwd => {
                println!("{}", self.working_dir.display());
            }
            Command::Exit => {
                process::exit(0);
            }
            Command::None(cmd_name) => {
                println!("{cmd_name}: command not found")
            }
        }
    }
    fn change_dir(&mut self, path: PathBuf) {
        self.working_dir = path;
    }
}
