use std::{collections::VecDeque, env, str::Chars};

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
    Chunking,
    ChunkReady,
    EndReached,
}

#[derive(Debug)]
pub struct MetaSymbolExpander<'a> {
    chars: Chars<'a>,
    out_queue: VecDeque<String>,
    temp_buffer: String,
    exansion_buffer: String,
    mode: MetaSymbolExpanderMode,
    active_separator: Option<Separator>,
    active_special: Option<MetaChar>,
}

impl<'a> MetaSymbolExpander<'a> {
    pub fn new(chars: Chars) -> MetaSymbolExpander {
        MetaSymbolExpander {
            chars,
            out_queue: VecDeque::with_capacity(2),
            temp_buffer: String::with_capacity(10),
            exansion_buffer: String::with_capacity(10),
            mode: MetaSymbolExpanderMode::Chunking,
            active_separator: None,
            active_special: None,
        }
    }

    fn process_next_char(&mut self) {
        let next_char = self.chars.next();
        if let None = next_char {
            self.mode = MetaSymbolExpanderMode::EndReached;
            return;
        }

        let fn_for_normal = |s: &mut Self, normal_char: char| {
            if s.active_special.is_some() {
                s.exansion_buffer.push(normal_char);
            } else {
                s.temp_buffer.push(normal_char);
            }
        };

        let fn_for_special = |s: &mut Self, special_char: MetaChar| {
            if s.active_separator
                .is_some_and(|s| s.allows_meta_char(&special_char))
                || s.active_separator.is_none()
            {
                if s.active_special.is_some() {
                    s.exansion_buffer.push(special_char.name())
                } else {
                    s.active_special = Some(special_char);
                }
            } else {
                s.temp_buffer.push(special_char.name());
            }
        };

        let fn_for_separator = |s: &mut Self, separator_char: Separator| {
            let Some(active_sep_char) = s.active_separator else {
                // No active_separator case
                if let Some(special_char) = s.active_special {
                    s.temp_buffer
                        .push_str(&special_char.expand(&s.exansion_buffer));
                    s.exansion_buffer.clear();
                    s.active_special = None;
                }

                if !s.temp_buffer.is_empty() {
                    s.out_queue.push_back(s.temp_buffer.clone());
                    s.temp_buffer.clear();
                }

                match separator_char {
                    Separator::Whitespace(_) => s.out_queue.push_back(" ".to_string()),
                    _ => {
                        s.active_separator = Some(separator_char);
                    }
                }

                if !s.out_queue.is_empty() {
                    s.mode = MetaSymbolExpanderMode::ChunkReady;
                }

                return;
            };

            if active_sep_char == separator_char {
                s.active_separator = None;
                if let Some(special_char) = s.active_special {
                    s.temp_buffer
                        .push_str(&special_char.expand(&s.exansion_buffer));
                    s.exansion_buffer.clear();
                    s.active_special = None;
                }
                if !s.temp_buffer.is_empty() {
                    s.out_queue.push_back(s.temp_buffer.clone());
                    s.temp_buffer.clear();
                    s.mode = MetaSymbolExpanderMode::ChunkReady;
                }
            } else {
                if s.active_special.is_some() {
                    s.exansion_buffer.push(separator_char.name());
                } else {
                    s.temp_buffer.push(separator_char.name());
                }
            }
        };

        if let MetaSymbolExpanderMode::Chunking = self.mode {
            self.act_on_special_or_separator_or_else(
                next_char,
                fn_for_special,
                fn_for_separator,
                fn_for_normal,
            );
        }
    }

    fn act_on_special_or_separator_or_else(
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
        'outer: while self.mode != MetaSymbolExpanderMode::EndReached {
            while self.mode != MetaSymbolExpanderMode::ChunkReady {
                if self.mode == MetaSymbolExpanderMode::EndReached {
                    break 'outer;
                }
                self.process_next_char();
            }

            self.mode = MetaSymbolExpanderMode::Chunking;

            return self.out_queue.pop_front();
        }

        if !&self.temp_buffer.is_empty() {
            self.out_queue.push_back(self.temp_buffer.clone());
            self.temp_buffer.clear();
        }

        return self.out_queue.pop_front();
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn expander_case1() {
        let input = "\"hello\" \"world\"";
        let input_iter = MetaSymbolExpander::new(input.chars());

        let actual: Vec<String> = input_iter.collect();
        let expected = vec!["hello".to_string(), " ".to_string(), "world".to_string()];

        assert_eq!(expected, actual, "\ninput: ${:#?}", input);
    }

    #[test]
    fn expander_case2() {
        let input = "\"hello\"  \"world\" memo";
        let input_iter = MetaSymbolExpander::new(input.chars());

        let actual: Vec<String> = input_iter.collect();
        let expected = vec![
            "hello".to_string(),
            " ".to_string(),
            "world".to_string(),
            " ".to_string(),
            "memo".to_string(),
        ];

        assert_eq!(expected, actual, "\ninput: ${:#?}", input);
    }
    #[test]
    fn expander_case3() {
        let input = "hello  world memo";
        let input_iter = MetaSymbolExpander::new(input.chars());

        let actual: Vec<String> = input_iter.collect();
        let expected = vec![
            "hello".to_string(),
            " ".to_string(),
            "world".to_string(),
            " ".to_string(),
            "memo".to_string(),
        ];

        assert_eq!(expected, actual, "\ninput: ${:#?}", input);
    }

    #[test]
    fn expander_case4() {
        let input = "hello  \"worl'd  memo\"";
        let input_iter = MetaSymbolExpander::new(input.chars());

        let actual: Vec<String> = input_iter.collect();
        let expected = vec![
            "hello".to_string(),
            " ".to_string(),
            "worl'd  memo".to_string(),
        ];

        assert_eq!(expected, actual, "\ninput: {:#?}", input);
    }

    #[test]
    fn expander_case5() {
        let input = "echo hello world";
        let input_iter = MetaSymbolExpander::new(input.chars());

        let actual: Vec<String> = input_iter.collect();
        let expected = vec![
            "echo".to_string(),
            " ".to_string(),
            "hello".to_string(),
            " ".to_string(),
            "world".to_string(),
        ];

        assert_eq!(expected, actual, "\ninput: {:#?}", input);
    }
}
