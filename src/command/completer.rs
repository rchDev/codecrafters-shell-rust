use rustyline::Context;
use rustyline::completion::{Completer, Pair};

pub struct CommandCompleter {
    knowledge_base: PrefixTree,
}

impl CommandCompleter {
    pub fn new(commands: &[&str]) -> CommandCompleter {
        let mut knowledge_base = PrefixTree::new();
        for item in commands {
            match knowledge_base.add(*item) {
                Ok(_) => {}
                Err(msg) => panic!("{}", msg),
            }
        }
        CommandCompleter { knowledge_base }
    }

    pub fn add_commands(&mut self, commands: &[&str]) -> Result<(), &'static str> {
        for command in commands {
            self.knowledge_base.add(command)?;
        }
        Ok(())
    }
}

impl rustyline::Helper for CommandCompleter {}
impl rustyline::highlight::Highlighter for CommandCompleter {}
impl rustyline::validate::Validator for CommandCompleter {}
impl rustyline::hint::Hinter for CommandCompleter {
    type Hint = String;
}

impl Completer for CommandCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        match self.knowledge_base.starts_with(line) {
            Some(mut results) => {
                results.sort();
                Ok((
                    pos,
                    results
                        .iter()
                        .map(|result| Pair {
                            display: String::from(result),
                            replacement: String::from(&result[pos..]) + " ",
                        })
                        .collect(),
                ))
            }
            None => Ok((
                pos,
                vec![Pair {
                    display: String::new(),
                    replacement: String::from("\x07"),
                }],
            )),
        }
    }
}

#[derive(Debug)]
struct PrefixTree {
    root_children: Vec<PrefixNode>,
}

#[derive(Debug, Clone)]
struct PrefixNode {
    data: u8,
    is_end: bool,
    children: Vec<PrefixNode>,
}

impl PrefixTree {
    fn new() -> PrefixTree {
        PrefixTree {
            root_children: Vec::new(),
        }
    }

    fn add(&mut self, items: &str) -> Result<(), &'static str> {
        let mut children = &mut self.root_children;
        let mut item_iter = items.as_bytes().iter().peekable();
        while let Some(item) = item_iter.next() {
            let item_is_last = item_iter.peek().is_none();

            let child_idx = children.iter().position(|c| c.data == *item);
            if let Some(idx) = child_idx {
                let child = &mut children[idx];
                if item_is_last {
                    if !child.is_end {
                        child.is_end = true;
                    }
                    return Ok(());
                }
                children = &mut child.children;
            } else {
                let index = children.len();
                let new_node = PrefixNode {
                    data: *item,
                    is_end: item_is_last,
                    children: Vec::new(),
                };
                children.push(new_node);
                children = &mut children[index].children;
            }
        }

        Ok(())
    }

    fn starts_with(&self, input: &str) -> Option<Vec<String>> {
        let mut results: Vec<String> = Vec::new();
        let mut children = &self.root_children;
        let input_bytes = input.as_bytes();
        let mut running_prefix: Vec<u8> = Vec::with_capacity(input_bytes.len());

        for byte in input_bytes {
            let matching_child = children.iter().find(|child| child.data == *byte);
            if let Some(matching_node) = matching_child {
                running_prefix.push(*byte);
                children = &matching_node.children;
                if input_bytes.len() == running_prefix.len() && matching_node.is_end {
                    results.push(String::from_utf8_lossy(&running_prefix).to_string());
                }
            } else {
                return None;
            }
        }

        let mut search_stack: Vec<(PrefixNode, Vec<u8>)> = Vec::with_capacity(10);
        for child in children {
            let path_to_child = Vec::from(running_prefix.clone());
            search_stack.push((child.clone(), path_to_child));
        }

        while !search_stack.is_empty() {
            if let Some((node, mut path_to_node)) = search_stack.pop() {
                path_to_node.push(node.data);
                if node.is_end {
                    results.push(String::from_utf8_lossy(&path_to_node).to_string());
                }
                for child in node.children {
                    search_stack.push((child, path_to_node.clone()));
                }
            }
        }

        if results.len() == 0 {
            return None;
        }

        Some(results)
    }
}

#[cfg(test)]
mod test {
    use crate::command::{BUILTIN_COMMAND_NAMES, completer::PrefixTree};

    #[test]
    fn prefix_tree_constructed_correctly() {
        let mut pt = PrefixTree::new();
        let first_str = String::from("Hello");
        let second_str = String::from("Halo");
        let third_str = String::from("Ham");

        let _ = pt.add(&first_str);
        let _ = pt.add(&second_str);
        let _ = pt.add(&third_str);

        dbg!(&pt);
    }

    #[test]
    fn prefix_tree_starts_with_works_correctly() {
        let mut pt = PrefixTree::new();

        for command in BUILTIN_COMMAND_NAMES {
            let _ = pt.add(command);
        }

        let test_input = String::from("e");
        if let Some(results) = pt.starts_with(&test_input) {
            dbg!(results);
        }

        let test_input = String::from("c");
        if let Some(results) = pt.starts_with(&test_input) {
            dbg!(results);
        }

        let test_input = String::from("cd");
        if let Some(results) = pt.starts_with(&test_input) {
            dbg!(results);
        }
    }
}
