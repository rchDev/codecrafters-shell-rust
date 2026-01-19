#[allow(unused_imports)]
use std::io::{self, Write, stdin};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::{env, fs, path::Path, process, str::FromStr};

fn main() {
    // TODO: Uncomment the code below to pass the first stage

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

        let input = input.trim();
        match Command::from_str(input) {
            Ok(command) => command.execute(),
            Err(e) => {
                eprintln!("{e}");
                continue 'main_loop;
            }
        };
    }
}

enum Command {
    Exit,
    Echo(String),
    Type(TypeItem),
}

enum TypeItem {
    BuiltIn(String),
    PathExec(String, PathBuf),
}

impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.split(" ");

        let Some(command) = s.next() else {
            return Err(": command not found".to_string());
        };

        let rest = s.collect::<Vec<&str>>();

        let command = match command {
            "exit" => Ok(Command::Exit),
            "echo" => Ok(Command::Echo(rest.join(" "))),
            "type" => {
                if rest.len() > 1 || rest.len() == 0 {
                    return Err(format!("{}: not found", rest.join(" ")));
                }

                let inner = rest[0];

                if Command::is_valid(inner) {
                    Ok(Command::Type(TypeItem::BuiltIn(inner.to_string())))
                } else {
                    get_executable_path(inner)
                        .map(|p| Command::Type(TypeItem::PathExec(inner.to_string(), p)))
                        .ok_or_else(|| format!("{inner}: not found"))
                }
            }
            other => Err(format!("{other}: command not found")),
        };
        command
    }
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
    fn is_valid(s: &str) -> bool {
        match s {
            "exit" | "echo" | "type" => true,
            _ => false,
        }
    }

    fn execute(&self) {
        match self {
            Command::Exit => {
                process::exit(0);
            }
            Command::Echo(str) => {
                println!("{str}")
            }
            Command::Type(TypeItem::BuiltIn(inner)) => {
                println!("{inner} is a shell builtin")
            }
            Command::Type(TypeItem::PathExec(command, path)) => {
                let path = path.to_str().unwrap_or_default();
                println!("{command} is {path}")
            }
        }
    }
}
