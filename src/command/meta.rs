use std::env;

#[derive(PartialEq, Debug)]
pub enum MetaChar {
    Dollar,
    Star,
    Tilde,
    Whitespace(char),
}

impl MetaChar {
    #[allow(dead_code)]
    pub fn name(&self) -> char {
        match self {
            MetaChar::Dollar => '$',
            MetaChar::Star => '*',
            MetaChar::Tilde => '~',
            MetaChar::Whitespace(val) => *val,
        }
    }

    pub fn expand(&self, expansion_buf: &mut String, out: &mut String) {
        match self {
            Self::Dollar => {
                println!("{expansion_buf}");
                let var = env::var(expansion_buf).unwrap();
                out.push_str(&var)
            }
            Self::Star => {}
            Self::Tilde => {}
            Self::Whitespace(_) => {}
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
            value if value.is_whitespace() => Ok(MetaChar::Whitespace(value)),
            _ => Err(()),
        }
    }
}

#[derive(PartialEq, Debug)]
pub enum ExpansionBlocker {
    Single,
    Double,
}

impl ExpansionBlocker {
    pub fn allows_meta_char(&self, meta_char: &MetaChar) -> bool {
        match self {
            Self::Single => false,
            Self::Double => match meta_char {
                MetaChar::Dollar => true,
                _ => false,
            },
        }
    }
}

impl TryFrom<char> for ExpansionBlocker {
    type Error = ();
    fn try_from(value: char) -> Result<Self, Self::Error> {
        match value {
            '\'' => Ok(Self::Single),
            '"' => Ok(Self::Double),
            _ => Err(()),
        }
    }
}
