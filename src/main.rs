#[allow(unused_imports)]
use std::io::{self, Write, stdin};
use std::process;

fn main() {
    // TODO: Uncomment the code below to pass the first stage
    print!("$ ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap_or_else(|err| {
        eprintln!("Failed to read user input: {err}");
        process::exit(1);
    });

    let _command = Command::build(input).unwrap_or_else(|err| {
        eprintln!("{err}");
        process::exit(1);
    });
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
