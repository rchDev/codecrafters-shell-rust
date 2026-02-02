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
    Chunking,
    ChunkReady,
    EndReached,
}

#[derive(Debug)]
pub struct MetaSymbolExpander<'a> {
    chars: Chars<'a>,
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

        let fn_for_normal = |s: &mut Self, normal_char: char| {};
        let fn_for_special = |s: &mut Self, special_char: MetaChar| {};
        let fn_for_separator = |s: &mut Self, separator_char: Separator| {};

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
