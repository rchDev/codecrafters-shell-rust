use std::env;

#[derive(PartialEq, Debug)]
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
                dbg!("IM HERE!!!");
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
