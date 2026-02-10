use codecrafters_shell::command::{self, BUILTIN_COMMAND_NAMES};
use codecrafters_shell::shell::{Command, CommandCompleter, Shell};
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use rustyline::{CompletionType, Config, Editor, Result};

#[allow(unused_imports)]
use std::io::{self, Write, stdin};

fn main() -> Result<()> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let external_commands = command::get_external_commands(path);
    let command_names: Vec<String> = external_commands
        .keys()
        .map(|key| key.clone().into_string())
        .filter_map(|key| key.ok())
        .collect();
    let command_name_refs: Vec<&str> = command_names.iter().map(String::as_str).collect();
    let mut autocompleter = CommandCompleter::new(&command_name_refs);
    if let Err(msg) = autocompleter.add_commands(BUILTIN_COMMAND_NAMES) {
        panic!("{}", msg);
    }
    let config = Config::builder()
        .completion_type(CompletionType::List) // or CompletionType::List
        .build();
    let mut rl: Editor<CommandCompleter, DefaultHistory> = Editor::with_config(config)?;
    rl.set_helper(Some(autocompleter));

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
