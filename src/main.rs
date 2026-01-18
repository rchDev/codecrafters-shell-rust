#[allow(unused_imports)]
use std::io::{self, Write, stdin};
use std::process;

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

        let _command = match Command::build(input) {
            Ok(command) => command,
            Err(e) => {
                eprintln!("{e}");
                continue 'main_loop;
            }
        };
    }
}

struct Command {
    name: String,
}

impl Command {
    fn build(input: String) -> Result<Self, String> {
        let command = input.trim();

        Err(format!("{command}: command not found"))
    }
}
