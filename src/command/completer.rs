use rustyline::Context;
use rustyline::completion::{Completer, Pair};

pub struct CommandCompleter<T: PartialEq> {
    corpus: PrefixTree<T>,
}

impl<T: PartialEq> CommandCompleter<T> {
    pub fn new() -> CommandCompleter<T> {
        CommandCompleter {
            corpus: PrefixTree::new(),
        }
    }
}

impl<T: PartialEq> rustyline::Helper for CommandCompleter<T> {}
impl<T: PartialEq> rustyline::highlight::Highlighter for CommandCompleter<T> {}
impl<T: PartialEq> rustyline::validate::Validator for CommandCompleter<T> {}
impl<T: PartialEq> rustyline::hint::Hinter for CommandCompleter<T> {
    type Hint = String;
}

impl<T: PartialEq> Completer for CommandCompleter<T> {
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

struct PrefixTree<T: PartialEq> {
    root_children: Vec<PrefixNode<T>>,
}

struct PrefixNode<T: PartialEq> {
    data: T,
    is_end: bool,
    children: Vec<PrefixNode<T>>,
}

impl<T: PartialEq> PrefixTree<T> {
    fn new() -> PrefixTree<T> {
        PrefixTree {
            root_children: Vec::new(),
        }
    }

    fn add(&mut self, item: T) -> Result<(), &'static str> {
        let children = &mut self.root_children;

        loop {
            for child in children.iter_mut() {
                if child.data == item {
                    if !child.is_end {
                        child.is_end = true;
                    }
                    return Ok(());
                }
            }
            children.push(PrefixNode {
                data: item,
                is_end: true,
                children: Vec::new(),
            });
            return Ok(());
        }
    }

    fn add_many<IT: Iterator<Item = T>>(&mut self, items: IT) -> Result<(), &'static str> {
        let mut children = &mut self.root_children;
        let mut item_iter = items.into_iter().peekable();
        while let Some(item) = item_iter.next() {
            let item_is_last = item_iter.peek().is_none();

            let child_idx = children.iter().position(|c| c.data == item);
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
                let new_node = PrefixNode {
                    data: item,
                    is_end: item_is_last,
                    children: Vec::new(),
                };
                children.push(new_node);
            }
        }

        Ok(())
    }
}
