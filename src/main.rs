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
        }
    }
}

enum Command {
    Exit,
    Echo(String),
}
impl FromStr for Command {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut s = s.trim().split(" ");

        let Some(command) = s.next() else {
            return Err(": command not found".to_string());
        };

        let rest = s.collect::<Vec<&str>>().join(" ");

        match command {
            "exit" => Ok(Command::Exit),
            "echo" => Ok(Command::Echo(rest)),
            other => Err(format!("{other}: command not found")),
        }
    }
}
