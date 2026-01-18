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
        }
    }
}

enum Command {
    Exit,
}
impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim() {
            "exit" => Ok(Command::Exit),
            other => Err(format!("command not found: {other}")),
        }
    }
}
