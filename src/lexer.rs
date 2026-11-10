use crate::error::DsError;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Ident(String),
    Number(u64),
    Str(String),
    LBrace,
    RBrace,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub line: usize,
    pub col: usize,
}

pub struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Self {
            chars: source.chars().collect(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, DsError> {
        let mut tokens = Vec::new();

        while let Some(c) = self.peek() {
            let (line, col) = (self.line, self.col);

            if c.is_whitespace() {
                self.bump();
            } else if c == '#' {
                // komentarz do końca linii
                while let Some(c) = self.peek() {
                    if c == '\n' {
                        break;
                    }
                    self.bump();
                }
            } else if c == '{' {
                self.bump();
                tokens.push(Token {
                    kind: TokenKind::LBrace,
                    line,
                    col,
                });
            } else if c == '}' {
                self.bump();
                tokens.push(Token {
                    kind: TokenKind::RBrace,
                    line,
                    col,
                });
            } else if c == '"' {
                self.bump();
                let mut text = String::new();
                loop {
                    match self.bump() {
                        Some('"') => break,
                        Some('\n') | None => {
                            return Err(DsError::new(
                                line,
                                col,
                                "niezamknięty tekst – brakuje zamykającego \"",
                            ));
                        }
                        Some(ch) => text.push(ch),
                    }
                }
                tokens.push(Token {
                    kind: TokenKind::Str(text),
                    line,
                    col,
                });
            } else if c.is_ascii_digit() {
                let mut digits = String::new();
                while let Some(d) = self.peek() {
                    if d.is_ascii_digit() {
                        digits.push(d);
                        self.bump();
                    } else {
                        break;
                    }
                }
                let n = digits
                    .parse::<u64>()
                    .map_err(|_| DsError::new(line, col, "liczba jest zbyt duża"))?;
                tokens.push(Token {
                    kind: TokenKind::Number(n),
                    line,
                    col,
                });
            } else if c.is_alphabetic() || c == '_' {
                let mut word = String::new();
                while let Some(ch) = self.peek() {
                    if ch.is_alphanumeric() || ch == '_' {
                        word.push(ch);
                        self.bump();
                    } else {
                        break;
                    }
                }
                tokens.push(Token {
                    kind: TokenKind::Ident(word),
                    line,
                    col,
                });
            } else {
                return Err(DsError::new(
                    line,
                    col,
                    format!("nieoczekiwany znak '{}'", c),
                ));
            }
        }

        tokens.push(Token {
            kind: TokenKind::Eof,
            line: self.line,
            col: self.col,
        });
        Ok(tokens)
    }
}
