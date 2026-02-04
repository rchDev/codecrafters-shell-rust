#[allow(unused_imports)]
use std::io::{self, Write, stdin};

use codecrafters_shell::shell::{Command, Shell};

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

        let commands = Command::parse(&input);
        shell.exec_command(commands);
    }
}
