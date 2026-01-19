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

        let mut args = input.trim().split(" ");
        let Some(search_item) = args.next() else {
            eprintln!(": command not found");
            continue 'main_loop;
        };

        match Command::from_str(search_item) {
            Ok(command) => command.execute(&args.collect()),
            Err(parsing_err) => match get_executable_path(search_item) {
                Some(exec_path) => {
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
                None => {
                    eprintln!("{parsing_err}");
                }
            },
        };
    }
}

enum Command {
    Exit,
    Echo,
    Type,
    Pwd,
}

impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "exit" => Ok(Command::Exit),
            "echo" => Ok(Command::Echo),
            "type" => Ok(Command::Type),
            "pwd" => Ok(Command::Pwd),
            other => Err(format!("{other}: command not found")),
        }
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
    fn execute(&self, args: &Vec<&str>) {
        match self {
            Command::Exit => {
                process::exit(0);
            }
            Command::Echo => {
                Self::execute_echo_command(args);
            }
            Command::Type => {
                Self::execute_type_command(args);
            }
            Command::Pwd => {
                let cwd = env::current_dir().unwrap();
                println!("{}", cwd.display())
            }
        }
    }

    fn execute_type_command(args: &Vec<&str>) -> () {
        if args.len() == 0 || args.len() > 1 {
            eprintln!("{}: not found", args.join(" "));
            return;
        }

        if let Ok(_) = Command::from_str(args[0]) {
            println!("{} is a shell builtin", args[0]);
            return;
        }

        match get_executable_path(args[0]) {
            Some(exec_path) => {
                println!("{} is {}", args[0], exec_path.display());
            }
            None => {
                eprintln!("{}: not found", args[0])
            }
        }
    }
    fn execute_echo_command(args: &Vec<&str>) {
        println!("{}", args.join(" "));
    }
}
