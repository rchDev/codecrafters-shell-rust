use rustyline::Context;
use rustyline::completion::{Completer, Pair};

pub struct CommandCompleter {
    corpus: PrefixTree,
}

impl CommandCompleter {
    pub fn new() -> CommandCompleter {
        let corpus = PrefixTree::new();
        CommandCompleter { corpus }
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
        Ok((
            pos,
            vec![Pair {
                display: String::new(),
                replacement: String::from("yoyoyo"),
            }],
        ))
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

    fn add(&mut self, items: &[u8]) -> Result<(), &'static str> {
        let mut children = &mut self.root_children;
        let mut item_iter = items.into_iter().peekable();
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

    fn starts_with(&self, byte_slice: &[u8]) -> Option<Vec<Vec<u8>>> {
        let mut results: Vec<Vec<u8>> = Vec::new();
        let mut children = &self.root_children;

        let mut running_prefix: Vec<u8> = Vec::with_capacity(byte_slice.len());

        for byte in byte_slice {
            let matching_child = children.iter().find(|child| child.data == *byte);
            if let Some(matching_node) = matching_child {
                running_prefix.push(*byte);
                children = &matching_node.children;
                if byte_slice.len() == running_prefix.len() && matching_node.is_end {
                    results.push(running_prefix.clone());
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
                    results.push(path_to_node.clone());
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
    use crate::command::completer::PrefixTree;

    #[test]
    fn prefix_tree_constructed_correctly() {
        let mut pt = PrefixTree::new();
        let first_str = "Hello";
        let second_str = "Halo";
        let third_str = "Ham";

        let _ = pt.add(first_str.as_bytes());
        let _ = pt.add(second_str.as_bytes());
        let _ = pt.add(third_str.as_bytes());

        dbg!(third_str, third_str.as_bytes());
        dbg!(&pt);
    }

    #[test]
    fn prefix_tree_starts_with_works_correctly() {
        let mut pt = PrefixTree::new();
        let first_str = "Hello";
        let second_str = "Halo";
        let third_str = "Ham";
        let another_str = "Helium";

        let _ = pt.add(first_str.as_bytes());
        let _ = pt.add(second_str.as_bytes());
        let _ = pt.add(third_str.as_bytes());
        let _ = pt.add(another_str.as_bytes());

        dbg!(third_str, third_str.as_bytes());
        dbg!(&pt);

        if let Some(results) = pt.starts_with("H".as_bytes()) {
            let results: Vec<String> = results
                .into_iter()
                .map(|bytes| String::from_utf8(bytes).unwrap())
                .collect();
            dbg!(results);
        }
    }
}
