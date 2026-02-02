use std::{collections::VecDeque, env, str::Chars};

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum SpecialChar {
    Dollar,
    Star,
    Tilde,
    Backslash,
}

impl SpecialChar {
    #[allow(dead_code)]
    pub fn name(&self) -> char {
        match self {
            Self::Dollar => '$',
            Self::Star => '*',
            Self::Tilde => '~',
            Self::Backslash => '\\',
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
            Self::Backslash => expansion_buf.to_owned(),
        }
    }
}

impl TryFrom<char> for SpecialChar {
    type Error = ();
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            '$' => Ok(Self::Dollar),
            '*' => Ok(Self::Star),
            '~' => Ok(Self::Tilde),
            '\\' => Ok(Self::Backslash),
            _ => Err(()),
        }
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum ZoningChar {
    SingleQuote,
    DoubleQuote,
}

impl ZoningChar {
    pub fn allows_special_char(&self, meta_char: &SpecialChar) -> bool {
        match self {
            Self::SingleQuote => false,
            Self::DoubleQuote => match meta_char {
                SpecialChar::Dollar => true,
                SpecialChar::Backslash => true,
                _ => false,
            },
        }
    }

    #[allow(dead_code)]
    pub fn name(&self) -> char {
        match self {
            Self::SingleQuote => '\'',
            Self::DoubleQuote => '"',
        }
    }
}

impl TryFrom<char> for ZoningChar {
    type Error = ();
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            '\'' => Ok(Self::SingleQuote),
            '"' => Ok(Self::DoubleQuote),
            _ => Err(()),
        }
    }
}

#[derive(PartialEq, Debug, Clone, Copy)]
pub enum Separator {
    Whitespace(char),
}

impl Separator {
    #[allow(dead_code)]
    pub fn name(&self) -> char {
        match self {
            Self::Whitespace(val) => *val,
        }
    }
}

impl TryFrom<char> for Separator {
    type Error = ();
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
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
    expansion_buffer: String,
    mode: MetaSymbolExpanderMode,
    active_zoning: Option<ZoningChar>,
    active_special: Option<SpecialChar>,
    dbg_run: usize,
}

impl<'a> MetaSymbolExpander<'a> {
    pub fn new(chars: Chars) -> MetaSymbolExpander {
        MetaSymbolExpander {
            dbg_run: 0,
            chars,
            temp_buffer: String::with_capacity(10),
            expansion_buffer: String::with_capacity(10),
            mode: MetaSymbolExpanderMode::Chunking,
            active_zoning: None,
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
            if let Some(special) = s.active_special {
                s.expansion_buffer.push(normal_char);
                if special == SpecialChar::Backslash {
                    s.temp_buffer.push_str(&special.expand(&s.expansion_buffer));
                    s.expansion_buffer.clear();
                    s.active_special = None;
                }
            } else {
                s.temp_buffer.push(normal_char);
            }
        };

        let fn_for_special = |s: &mut Self, special_char: SpecialChar| {
            if s.active_zoning
                .is_some_and(|s| s.allows_special_char(&special_char))
                || s.active_zoning.is_none()
            {
                if let Some(active_spec_char) = s.active_special {
                    s.expansion_buffer.push(special_char.name());
                    if let SpecialChar::Backslash = active_spec_char {
                        s.temp_buffer
                            .push_str(&special_char.expand(&s.expansion_buffer));
                        s.expansion_buffer.clear();
                        s.active_special = None;
                    }
                } else if let SpecialChar::Tilde = special_char {
                    s.temp_buffer.push_str(&special_char.expand(""));
                } else {
                    s.active_special = Some(special_char);
                }
            } else {
                s.temp_buffer.push(special_char.name());
            }
        };

        let fn_for_zoning = |s: &mut Self, zoning_char: ZoningChar| {
            if let Some(current_zoning_char) = s.active_zoning {
                if current_zoning_char == zoning_char {
                    if let Some(special_char) = s.active_special {
                        if special_char == SpecialChar::Backslash {
                            s.expansion_buffer.push(zoning_char.name());
                        }
                        s.temp_buffer
                            .push_str(&special_char.expand(&s.expansion_buffer));
                        s.expansion_buffer.clear();
                        s.active_special = None;
                    } else {
                        s.active_zoning = None;
                    }
                } else {
                    s.temp_buffer.push(zoning_char.name());
                }
            } else {
                if let Some(special_char) = s.active_special {
                    if special_char == SpecialChar::Backslash {
                        s.expansion_buffer.push(zoning_char.name());
                    } else {
                        s.active_zoning = Some(zoning_char);
                    }
                    s.temp_buffer
                        .push_str(&special_char.expand(&s.expansion_buffer));
                    s.expansion_buffer.clear();
                    s.active_special = None;
                } else {
                    s.active_zoning = Some(zoning_char);
                }
            }
        };

        let fn_for_separator = |s: &mut Self, separator: Separator| {
            if let Some(special_char) = s.active_special {
                let special_char_is_backslash = special_char == SpecialChar::Backslash;
                if special_char_is_backslash {
                    s.expansion_buffer.push(separator.name());
                }

                s.temp_buffer
                    .push_str(&special_char.expand(&s.expansion_buffer));
                s.expansion_buffer.clear();
                s.active_special = None;

                if special_char_is_backslash {
                    return;
                }
            }
            if s.active_zoning.is_some() {
                s.temp_buffer.push(separator.name());
            } else if !s.temp_buffer.is_empty() {
                s.mode = MetaSymbolExpanderMode::ChunkReady;
            }
        };
        self.apply_special_or_meta_or_separator_or_else(
            next_char,
            fn_for_special,
            fn_for_zoning,
            fn_for_separator,
            fn_for_normal,
        );
    }

    fn apply_special_or_meta_or_separator_or_else(
        &mut self,
        character: Option<char>,
        mut fn_for_meta: impl FnMut(&mut Self, SpecialChar),
        mut fn_for_zoning: impl FnMut(&mut Self, ZoningChar),
        mut fn_for_separator: impl FnMut(&mut Self, Separator),
        mut fn_for_else: impl FnMut(&mut Self, char),
    ) {
        if let Some(c) = character {
            if let Ok(meta_char) = SpecialChar::try_from(c) {
                fn_for_meta(self, meta_char);
            } else if let Ok(zoning_char) = ZoningChar::try_from(c) {
                fn_for_zoning(self, zoning_char);
            } else if let Ok(separator) = Separator::try_from(c) {
                fn_for_separator(self, separator);
            } else {
                fn_for_else(self, c);
            }
        }

        self.dbg_run += 1;
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

            let res = self.temp_buffer.clone();
            self.temp_buffer.clear();
            return Some(res);
        }

        if !&self.temp_buffer.is_empty() {
            let res = self.temp_buffer.clone();
            self.temp_buffer.clear();
            return Some(res);
        }

        None
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
        let expected = vec!["hello".to_string(), "world".to_string()];

        assert_eq!(expected, actual, "\ninput: ${:#?}", input);
    }

    #[test]
    fn expander_case2() {
        let input = "\"hello\"  \"world\" memo";
        let input_iter = MetaSymbolExpander::new(input.chars());

        let actual: Vec<String> = input_iter.collect();
        let expected = vec!["hello".to_string(), "world".to_string(), "memo".to_string()];

        assert_eq!(expected, actual, "\ninput: ${:#?}", input);
    }
    #[test]
    fn expander_case3() {
        let input = "hello  world memo";
        let input_iter = MetaSymbolExpander::new(input.chars());

        let actual: Vec<String> = input_iter.collect();
        let expected = vec!["hello".to_string(), "world".to_string(), "memo".to_string()];

        assert_eq!(expected, actual, "\ninput: ${:#?}", input);
    }

    #[test]
    fn expander_case4() {
        let input = "hello  \"worl'd  memo\"";
        let input_iter = MetaSymbolExpander::new(input.chars());

        let actual: Vec<String> = input_iter.collect();
        let expected = vec!["hello".to_string(), "worl'd  memo".to_string()];

        assert_eq!(expected, actual, "\ninput: {:#?}", input);
    }

    #[test]
    fn expander_case5() {
        let input = "echo hello world";
        let input_iter = MetaSymbolExpander::new(input.chars());

        let actual: Vec<String> = input_iter.collect();
        let expected = vec!["echo".to_string(), "hello".to_string(), "world".to_string()];

        assert_eq!(expected, actual, "\ninput: {:#?}", input);
    }

    #[test]
    fn expander_case6() {
        let input =
            r#"cat /tmp/rat/'no slash 44' /tmp/rat/'one slash \95' /tmp/rat/'two slashes \50\'"#;

        let input_iter = MetaSymbolExpander::new(input.chars());

        let actual: Vec<String> = input_iter.collect();
        let expected = vec![
            "cat".to_string(),
            "/tmp/rat/no slash 44".to_string(),
            r#"/tmp/rat/one slash \95"#.to_string(),
            r#"/tmp/rat/two slashes \50\"#.to_string(),
        ];
        assert_eq!(expected, actual, "\ninput: {:#?}", input);
    }

    #[test]
    fn expander_case7() {
        let input = r#"\'\"test hello\"\'"#;

        let input_iter = MetaSymbolExpander::new(input.chars());

        let actual: Vec<String> = input_iter.collect();
        let expected = vec!["'\"test".to_string(), "hello\"'".to_string()];
        assert_eq!(expected, actual, "\ninput: {:#?}", input);
    }

    #[test]
    fn expander_case8() {
        let input = r#""mixed\"quote'example'\\"#;

        let input_iter = MetaSymbolExpander::new(input.chars());

        let actual: String = input_iter.collect::<Vec<String>>().join("");
        let expected = "mixed\"quote'example'\\".to_string();

        assert_eq!(expected, actual, "\ninput: {:#?}", input);
    }
}
