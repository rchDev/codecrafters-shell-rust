#[allow(unused_imports)]
use std::io::{self, Write, stdin};
use std::{process, str::FromStr};

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

        let command = match Command::from_str(&input) {
            Ok(command) => command,
            Err(e) => {
                eprintln!("{e}");
                continue 'main_loop;
            }
        };

        match command {
            Command::Exit => {
                process::exit(0);
            }
            Command::Echo(str) => {
                println!("{str}")
            }
            Command::Type(inner) => {
                println!("{inner} is a shell builtin")
            }
        }
    }
}

enum Command {
    Exit,
    Echo(String),
    Type(String),
}
impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.trim().split(" ");

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
                if !Command::is_valid(inner) {
                    return Err(format!("{inner}: not found"));
                }

                Ok(Command::Type(inner.to_string()))
            }
            other => Err(format!("{other}: command not found")),
        };
        command
    }
}

impl Command {
    fn is_valid(s: &str) -> bool {
        match s {
            "exit" | "echo" | "type" => true,
            _ => false,
        }
    }
}
