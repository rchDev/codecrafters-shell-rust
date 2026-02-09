#[allow(unused_imports)]
use std::io::{self, Write, stdin};

use codecrafters_shell::command;
use codecrafters_shell::shell::{Command, CommandCompleter, Shell};
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use rustyline::{Editor, Result};

fn main() -> Result<()> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let external_commands = command::get_external_commands(path);
    let mut rl: Editor<CommandCompleter, DefaultHistory> = Editor::new()?;
    rl.set_helper(Some(CommandCompleter::new()));

    let mut shell = Shell::new();

    loop {
        let readline = rl.readline("$ ");
        match readline {
            Ok(line) => {
                let command_result = Command::parse(&line);
                shell.exec_command(command_result);
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }

    Ok(())
}
