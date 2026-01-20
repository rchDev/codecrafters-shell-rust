use std::fmt;
#[allow(unused_imports)]
use std::io::{self, Write, stdin};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::{env, fs, path::Path, process};

fn main() {
    // TODO: Uncomment the code below to pass the first stage
    let mut shell = Shell::new();
    'main_loop: loop {
        print!("$ ");
        io::stdout().flush().unwrap();

        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Failed to read user input: {e}");
                continue 'main_loop;
            }
        };

        let cmd = Command::parse(&input);
        shell.exec_command(cmd);
    }
}

enum Command {
    Exit,
    Echo(String),
    Type(Box<Command>),
    Pwd,
    Cd(PathBuf),
    External {
        exec_path: PathBuf,
        args: Vec<String>,
    },
    None(String),
}

fn get_executable_path(input: &str) -> Option<PathBuf> {
    let path = env::var_os("PATH").unwrap_or_default();
    for dir in env::split_paths(&path) {
        let exec_path = dir.join(input);
        if is_executable(&exec_path) {
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

impl Command {
    fn parse(s: &str) -> Command {
        let mut args = s.trim().split(" ");
        let Some(cmd) = args.next() else {
            return Command::None("".to_string());
        };
        let args: Vec<&str> = args.collect();
        match cmd {
            "exit" => Command::Exit,
            "echo" => Command::Echo(args.join(" ")),
            "type" => {
                if args.len() == 0 || args.len() > 1 {
                    return Command::Type(Box::new(Command::None(args.join(" "))));
                }
                let inner_cmd = Command::parse(args[0]);
                Command::Type(Box::new(inner_cmd))
            }
            "pwd" => Command::Pwd,
            "cd" => Command::Cd(PathBuf::from(args.join(" "))),
            other => match get_executable_path(other) {
                Some(exec_path) => Command::External {
                    exec_path,
                    args: args.into_iter().map(String::from).collect(),
                },
                None => Command::None(s.trim().to_string()),
            },
        }
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
struct Shell {
    working_dir: PathBuf,
}

impl Shell {
    /// Returns
    pub fn new() -> Self {
        Self {
            working_dir: env::current_dir().unwrap(),
        }
    }

    pub fn exec_command(&mut self, cmd: Command) {
        match cmd {
            Command::Cd(mut exec_path) => {
                if exec_path.eq("~") {
                    exec_path = env::home_dir().unwrap()
                }
                match env::set_current_dir(&exec_path) {
                    Ok(_) => self.change_dir(env::current_dir().unwrap()),
                    Err(_) => {
                        eprintln!("cd: {}: No such file or directory", exec_path.display())
                    }
                };
            }
            Command::Echo(msg) => {
                println!("{msg}");
            }
            Command::External { exec_path, args } => {
                let filename = exec_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or_default();

                let _ = process::Command::new(filename)
                    .args(args)
                    .stdin(process::Stdio::inherit())
                    .stdout(process::Stdio::inherit())
                    .stderr(process::Stdio::inherit())
                    .status();
            }
            Command::Type(inner) => match inner.as_ref() {
                Command::None(name) => println!("{name}: not found"),
                Command::External { exec_path, args: _ } => println!(
                    "{} is {}",
                    exec_path.file_name().unwrap_or_default().display(),
                    exec_path.display()
                ),
                builtin => println!("{builtin} is a shell builtin"),
            },
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
