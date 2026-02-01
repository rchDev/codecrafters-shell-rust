use std::{env, str::Chars};

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum MetaChar {
    Dollar,
    Star,
    Tilde,
}

impl MetaChar {
    #[allow(dead_code)]
    pub fn name(&self) -> char {
        match self {
            MetaChar::Dollar => '$',
            MetaChar::Star => '*',
            MetaChar::Tilde => '~',
        }
    }

    pub fn expand(&self, expansion_buf: &str) -> String {
        match self {
            Self::Dollar => env::var(expansion_buf).unwrap(),
            Self::Star => self.name().to_string(),
            Self::Tilde => {
                let home_dir = env::home_dir().unwrap();
                home_dir.into_os_string().into_string().unwrap()
            }
        }
    }
}

impl TryFrom<char> for MetaChar {
    type Error = ();
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            '$' => Ok(MetaChar::Dollar),
            '*' => Ok(MetaChar::Star),
            '~' => Ok(MetaChar::Tilde),
            _ => Err(()),
        }
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Separator {
    Single,
    Double,
    Whitespace(char),
}

impl Separator {
    pub fn allows_meta_char(&self, meta_char: &MetaChar) -> bool {
        match self {
            Self::Single => false,
            Self::Double => match meta_char {
                MetaChar::Dollar => true,
                _ => false,
            },
            Self::Whitespace(_) => true,
        }
    }

    #[allow(dead_code)]
    pub fn name(&self) -> char {
        match self {
            Self::Single => '\'',
            Self::Double => '"',
            Self::Whitespace(val) => *val,
        }
    }
}

impl TryFrom<char> for Separator {
    type Error = ();
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            '\'' => Ok(Self::Single),
            '"' => Ok(Self::Double),
            _ if value.is_whitespace() => Ok(Self::Whitespace(value)),
            _ => Err(()),
        }
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
enum MetaSymbolExpanderMode {
    Uninitialized,
    End,
    Regular,
    ChunkReady,
    ExpandingSpecial(MetaChar),
    Separating(Separator),
}

#[derive(Debug)]
pub struct MetaSymbolExpander<'a> {
    chars: Chars<'a>,
    temp_buffer: String,
    exansion_buffer: String,
    previous_mode: MetaSymbolExpanderMode,
    current_mode: MetaSymbolExpanderMode,
}

impl<'a> MetaSymbolExpander<'a> {
    pub fn new(chars: Chars) -> MetaSymbolExpander {
        MetaSymbolExpander {
            chars,
            temp_buffer: String::with_capacity(10),
            exansion_buffer: String::with_capacity(10),
            previous_mode: MetaSymbolExpanderMode::Uninitialized,
            current_mode: MetaSymbolExpanderMode::Uninitialized,
        }
    }

    fn next_mode(&mut self) {
        let next_char = self.chars.next();

        if let None = next_char {
            self.previous_mode = self.current_mode;
            self.current_mode = MetaSymbolExpanderMode::End;
            return;
        }

        match self.current_mode {
            MetaSymbolExpanderMode::Uninitialized => self.next_mode_from_uninitialized(next_char),
            MetaSymbolExpanderMode::End => {}
            MetaSymbolExpanderMode::Regular => self.next_mode_from_regular(next_char),
            MetaSymbolExpanderMode::ExpandingSpecial(meta_char) => {
                self.next_mode_from_expanding(meta_char, next_char)
            }
            MetaSymbolExpanderMode::Separating(sep) => {
                self.next_mode_from_separating(sep, next_char)
            }
            MetaSymbolExpanderMode::ChunkReady => {}
        }
    }

    fn next_mode_from_uninitialized(&mut self, character: Option<char>) {
        let fn_for_meta = |s: &mut Self, meta_char: MetaChar| {
            s.previous_mode = s.current_mode;
            s.current_mode = MetaSymbolExpanderMode::ExpandingSpecial(meta_char);
        };
        let fn_for_separator = |s: &mut Self, separator: Separator| {
            s.previous_mode = s.current_mode;
            s.current_mode = MetaSymbolExpanderMode::Separating(separator);
        };
        let fn_for_else = |s: &mut Self, x: char| {
            s.previous_mode = s.current_mode;
            s.current_mode = MetaSymbolExpanderMode::Regular;
            s.temp_buffer.push(x);
        };

        self.act_on_meta_or_separator_or_else(
            character,
            fn_for_meta,
            fn_for_separator,
            fn_for_else,
        );
    }

    fn next_mode_from_regular(&mut self, character: Option<char>) {
        let fn_for_meta = |s: &mut Self, meta_char: MetaChar| {
            if !s.temp_buffer.is_empty() {
                s.previous_mode = MetaSymbolExpanderMode::ExpandingSpecial(meta_char);
                s.current_mode = MetaSymbolExpanderMode::ChunkReady;
            } else {
                s.previous_mode = s.current_mode;
                s.current_mode = MetaSymbolExpanderMode::ExpandingSpecial(meta_char);
            }
        };

        let fn_for_separator = |s: &mut Self, separator: Separator| {
            if !s.temp_buffer.is_empty() {
                s.previous_mode = MetaSymbolExpanderMode::Separating(separator);
                s.current_mode = MetaSymbolExpanderMode::ChunkReady;
            } else {
                s.previous_mode = s.current_mode;
                s.current_mode = MetaSymbolExpanderMode::Separating(separator);
            }
        };

        let fn_for_else = |s: &mut Self, x: char| {
            s.temp_buffer.push(x);
        };

        self.act_on_meta_or_separator_or_else(
            character,
            fn_for_meta,
            fn_for_separator,
            fn_for_else,
        );
    }

    fn next_mode_from_expanding(&mut self, meta_char: MetaChar, character: Option<char>) {
        let fn_for_meta = |s: &mut Self, meta_char: MetaChar| {
            s.exansion_buffer.push(meta_char.name());
        };

        let fn_for_else = |s: &mut Self, x: char| {
            s.exansion_buffer.push(x);
        };

        let fn_for_separator = |s: &mut Self, separator: Separator| {
            let prev_separator = match s.previous_mode {
                MetaSymbolExpanderMode::Separating(sep) => Some(sep),
                _ => None,
            };
            match (separator, prev_separator) {
                (Separator::Whitespace(ws_symbol), None | Some(Separator::Whitespace(_))) => {
                    s.temp_buffer
                        .push_str(&meta_char.expand(&s.exansion_buffer));
                    s.temp_buffer.push(ws_symbol);
                    s.exansion_buffer.clear();
                    s.current_mode = MetaSymbolExpanderMode::ChunkReady;
                    s.previous_mode =
                        MetaSymbolExpanderMode::Separating(Separator::Whitespace(' '));
                }
                (Separator::Whitespace(ws_symbol), Some(Separator::Double | Separator::Single)) => {
                    s.temp_buffer
                        .push_str(&meta_char.expand(&s.exansion_buffer));
                    s.temp_buffer.push(ws_symbol);
                    s.exansion_buffer.clear();
                    s.current_mode =
                        MetaSymbolExpanderMode::Separating(Separator::Whitespace(ws_symbol));
                }
                (Separator::Single, Some(Separator::Single))
                | (Separator::Double, Some(Separator::Double)) => {
                    s.current_mode = MetaSymbolExpanderMode::ChunkReady;
                    s.previous_mode = MetaSymbolExpanderMode::Regular;
                    s.temp_buffer
                        .push_str(&meta_char.expand(&s.exansion_buffer));
                    s.exansion_buffer.clear();
                }
                (Separator::Single, None | Some(Separator::Whitespace(_)))
                | (Separator::Double, None | Some(Separator::Whitespace(_))) => {
                    s.current_mode = MetaSymbolExpanderMode::ChunkReady;
                    s.previous_mode = MetaSymbolExpanderMode::Separating(separator);
                    s.temp_buffer
                        .push_str(&meta_char.expand(&s.exansion_buffer));
                    s.exansion_buffer.clear();
                }
                (Separator::Single, Some(Separator::Double))
                | (Separator::Double, Some(Separator::Single)) => {
                    s.exansion_buffer.push(separator.name());
                }
            }
        };

        self.act_on_meta_or_separator_or_else(
            character,
            fn_for_meta,
            fn_for_separator,
            fn_for_else,
        );
    }

    fn next_mode_from_separating(&mut self, sep: Separator, character: Option<char>) {
        let fn_for_meta = |s: &mut Self, meta_char: MetaChar| {
            if sep.allows_meta_char(&meta_char) {
                s.previous_mode = MetaSymbolExpanderMode::Separating(sep);
                s.current_mode = MetaSymbolExpanderMode::ExpandingSpecial(meta_char)
            } else {
                s.temp_buffer.push(meta_char.name());
            }
        };

        let fn_for_else = |s: &mut Self, x: char| {
            s.temp_buffer.push(x);
        };

        let fn_for_separator = |s: &mut Self, new_sep: Separator| {
            match (sep, new_sep) {
                (Separator::Double, Separator::Double) | (Separator::Single, Separator::Single) => {
                    s.previous_mode = s.current_mode;
                    s.current_mode = MetaSymbolExpanderMode::ChunkReady;
                }
                (Separator::Double, Separator::Single) | (Separator::Single, Separator::Double) => {
                    s.temp_buffer.push(new_sep.name());
                }
                (Separator::Double | Separator::Single, Separator::Whitespace(_)) => {
                    s.temp_buffer.push(new_sep.name())
                }
                (Separator::Whitespace(_), Separator::Whitespace(_)) => {}
                (Separator::Whitespace(_), Separator::Double) => {}
                (Separator::Whitespace(_), Separator::Single) => {}
            };
        };

        self.act_on_meta_or_separator_or_else(
            character,
            fn_for_meta,
            fn_for_separator,
            fn_for_else,
        );
    }

    fn act_on_meta_or_separator_or_else(
        &mut self,
        character: Option<char>,
        mut fn_for_meta: impl FnMut(&mut Self, MetaChar),
        mut fn_for_separator: impl FnMut(&mut Self, Separator),
        mut fn_for_else: impl FnMut(&mut Self, char),
    ) {
        if let Some(c) = character {
            if let Ok(meta_char) = MetaChar::try_from(c) {
                fn_for_meta(self, meta_char);
            } else if let Ok(separator) = Separator::try_from(c) {
                fn_for_separator(self, separator);
            } else {
                fn_for_else(self, c);
            }
        }
    }
}

impl<'a> Iterator for MetaSymbolExpander<'a> {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        match self.current_mode {
            MetaSymbolExpanderMode::Separating(Separator::Whitespace(_)) => {
                self.previous_mode = self.current_mode;
                self.current_mode = MetaSymbolExpanderMode::Regular;

                return Some(" ".to_string());
            }
            MetaSymbolExpanderMode::End => {
                return None;
            }
            _ => {}
        }

        'outer: while self.current_mode != MetaSymbolExpanderMode::End {
            while self.current_mode != MetaSymbolExpanderMode::ChunkReady {
                if self.current_mode == MetaSymbolExpanderMode::End {
                    break 'outer;
                }
                self.next_mode();
            }

            self.current_mode = self.previous_mode;
            self.previous_mode = MetaSymbolExpanderMode::ChunkReady;
            let res = Some(self.temp_buffer.clone());
            self.temp_buffer.clear();
            return res;
        }

        if self.temp_buffer.is_empty() {
            return None;
        }

        let res = Some(self.temp_buffer.clone());
        self.temp_buffer.clear();
        res
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn expander_case1() {
        let input = "\"hello\" \"world\"";
        let input_iter = MetaSymbolExpander::new(input.chars());

        let collected_input: Vec<String> = input_iter.collect();
        let actual = vec!["hello".to_string(), " ".to_string(), "world".to_string()];

        assert_eq!(collected_input, actual);
    }

    #[test]
    fn expanded_case_2() {
        let input = "\"hello\"  \"world\" memo";
        let input_iter = MetaSymbolExpander::new(input.chars());

        let collected_input: Vec<String> = input_iter.collect();
        let actual = vec![
            "hello".to_string(),
            " ".to_string(),
            "world".to_string(),
            " ".to_string(),
            "memo".to_string(),
        ];

        assert_eq!(collected_input, actual);
    }
    #[test]
    fn expanded_case_3() {
        let input = "hello  world memo";
        let input_iter = MetaSymbolExpander::new(input.chars());

        let collected_input: Vec<String> = input_iter.collect();
        let actual = vec![
            "hello".to_string(),
            " ".to_string(),
            "world".to_string(),
            " ".to_string(),
            "memo".to_string(),
        ];

        assert_eq!(collected_input, actual);
    }

    #[test]
    fn expanded_case_4() {
        let input = "hello  worl'd memo";
        let input_iter = MetaSymbolExpander::new(input.chars());

        let collected_input: Vec<String> = input_iter.collect();
        let actual = vec![
            "hello".to_string(),
            " ".to_string(),
            "worl'd".to_string(),
            " ".to_string(),
            "memo".to_string(),
        ];

        assert_eq!(collected_input, actual);
    }

    #[test]
    fn expanded_case_5() {
        let input = "hello  \"worl'd  memo\"";
        let input_iter = MetaSymbolExpander::new(input.chars());

        let collected_input: Vec<String> = input_iter.collect();
        let actual = vec![
            "hello".to_string(),
            " ".to_string(),
            "worl'd  memo".to_string(),
        ];

        assert_eq!(collected_input, actual);
    }
}
