use crate::ast::*;
use crate::error::DsError;
use crate::lexer::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

fn describe(kind: &TokenKind) -> String {
    match kind {
        TokenKind::Ident(s) => format!("'{}'", s),
        TokenKind::Number(n) => format!("liczba {}", n),
        TokenKind::Str(s) => format!("tekst \"{}\"", s),
        TokenKind::LBrace => "'{'".to_string(),
        TokenKind::RBrace => "'}'".to_string(),
        TokenKind::Eof => "koniec pliku".to_string(),
    }
}

fn fail<T>(tok: &Token, msg: impl Into<String>) -> Result<T, DsError> {
    Err(DsError::new(tok.line, tok.col, msg))
}

fn pos_of(tok: &Token) -> Pos {
    Pos {
        line: tok.line,
        col: tok.col,
    }
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn next(&mut self) -> Token {
        let t = self.peek().clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, kind: TokenKind, what: &str) -> Result<Token, DsError> {
        let t = self.next();
        if t.kind == kind {
            Ok(t)
        } else {
            fail(
                &t,
                format!("oczekiwano {}, a jest {}", what, describe(&t.kind)),
            )
        }
    }

    fn expect_ident(&mut self, what: &str) -> Result<(String, Token), DsError> {
        let t = self.next();
        match &t.kind {
            TokenKind::Ident(s) => Ok((s.clone(), t.clone())),
            other => fail(
                &t,
                format!("oczekiwano {}, a jest {}", what, describe(other)),
            ),
        }
    }

    pub fn parse_script(&mut self) -> Result<Script, DsError> {
        let mut disks = Vec::new();
        while self.peek().kind != TokenKind::Eof {
            let (kw, tok) = self.expect_ident("słowa 'disk'")?;
            if kw != "disk" {
                return fail(&tok, format!("oczekiwano 'disk', a jest '{}'", kw));
            }
            disks.push(self.parse_disk(pos_of(&tok))?);
        }
        Ok(Script { disks })
    }

    fn parse_disk(&mut self, pos: Pos) -> Result<Disk, DsError> {
        let t = self.next();
        let number = match &t.kind {
            TokenKind::Number(n) if *n <= u32::MAX as u64 => *n as u32,
            _ => return fail(&t, "po 'disk' oczekiwano numeru dysku, np. disk 1"),
        };
        let number_pos = pos_of(&t);
        self.expect(TokenKind::LBrace, "'{'")?;

        let mut table = None;
        let mut partitions = Vec::new();

        loop {
            let t = self.next();
            match &t.kind {
                TokenKind::RBrace => break,
                TokenKind::Ident(kw) => match kw.as_str() {
                    "table" => {
                        if table.is_some() {
                            return fail(&t, "'table' podano dwa razy");
                        }
                        table = Some(self.parse_table()?);
                    }
                    "partition" => partitions.push(self.parse_partition()?),
                    other => {
                        return fail(
                            &t,
                            format!(
                            "nieznane polecenie '{}' w bloku dysku (dozwolone: table, partition)",
                            other
                        ),
                        )
                    }
                },
                other => {
                    return fail(
                        &t,
                        format!("nieoczekiwane {} w bloku dysku", describe(other)),
                    )
                }
            }
        }

        Ok(Disk {
            number,
            number_pos,
            pos,
            table,
            partitions,
        })
    }

    fn parse_table(&mut self) -> Result<TableKind, DsError> {
        let (v, t) = self.expect_ident("typu tablicy (gpt lub mbr)")?;
        match v.to_lowercase().as_str() {
            "gpt" => Ok(TableKind::Gpt),
            "mbr" => Ok(TableKind::Mbr),
            _ => fail(
                &t,
                format!("nieznany typ tablicy '{}' (dozwolone: gpt, mbr)", v),
            ),
        }
    }

    /// Zwraca rozmiar oraz pozycję jego wartości.
    fn parse_size(&mut self) -> Result<(Size, Pos), DsError> {
        let t = self.next();
        let pos = pos_of(&t);
        match &t.kind {
            TokenKind::Ident(s) if s == "rest" => Ok((Size::Rest, pos)),
            TokenKind::Number(n) => {
                if *n == 0 {
                    return fail(&t, "rozmiar partycji musi być większy od zera");
                }
                let (unit, ut) = self.expect_ident("jednostki (MB, GB lub TB)")?;
                let mult: u64 = match unit.to_uppercase().as_str() {
                    "MB" => 1,
                    "GB" => 1024,
                    "TB" => 1024 * 1024,
                    _ => {
                        return fail(
                            &ut,
                            format!("nieznana jednostka '{}' (dozwolone: MB, GB, TB)", unit),
                        )
                    }
                };
                match n.checked_mul(mult) {
                    Some(mb) => Ok((Size::Mb(mb), pos)),
                    None => fail(&t, "rozmiar jest zbyt duży"),
                }
            }
            other => fail(
                &t,
                format!(
                    "po 'size' oczekiwano rozmiaru (np. 100GB) lub 'rest', a jest {}",
                    describe(other)
                ),
            ),
        }
    }

    fn parse_partition(&mut self) -> Result<Partition, DsError> {
        let (name, name_tok) = self.expect_ident("nazwy partycji")?;
        let name_pos = pos_of(&name_tok);
        self.expect(TokenKind::LBrace, "'{'")?;

        let mut size: Option<(Size, Pos)> = None;
        let mut fs: Option<(FsKind, Pos)> = None;
        let mut label: Option<(String, Pos)> = None;
        let mut letter: Option<(char, Pos)> = None;

        loop {
            let t = self.next();
            match &t.kind {
                TokenKind::RBrace => break,
                TokenKind::Ident(kw) => {
                    match kw.as_str() {
                        "size" => {
                            if size.is_some() {
                                return fail(&t, "'size' podano dwa razy");
                            }
                            size = Some(self.parse_size()?);
                        }
                        "format" => {
                            if fs.is_some() {
                                return fail(&t, "'format' podano dwa razy");
                            }
                            let (v, vt) = self.expect_ident("systemu plików")?;
                            let kind = match v.to_lowercase().as_str() {
                            "ntfs" => FsKind::Ntfs,
                            "fat32" => FsKind::Fat32,
                            "exfat" => FsKind::Exfat,
                            "refs" => FsKind::Refs,
                            _ => {
                                return fail(
                                    &vt,
                                    format!("nieznany format '{}' (dozwolone: ntfs, fat32, exfat, refs)", v),
                                )
                            }
                        };
                            fs = Some((kind, pos_of(&vt)));
                        }
                        "label" => {
                            if label.is_some() {
                                return fail(&t, "'label' podano dwa razy");
                            }
                            let s = self.next();
                            match &s.kind {
                            TokenKind::Str(v) => label = Some((v.clone(), pos_of(&s))),
                            _ => return fail(&s, "po 'label' oczekiwano tekstu w cudzysłowie, np. label \"Dane\""),
                        }
                        }
                        "letter" => {
                            if letter.is_some() {
                                return fail(&t, "'letter' podano dwa razy");
                            }
                            let (v, vt) = self.expect_ident("litery dysku")?;
                            let mut cs = v.chars();
                            match (cs.next(), cs.next()) {
                                (Some(c), None) if c.is_ascii_alphabetic() => {
                                    letter = Some((c.to_ascii_uppercase(), pos_of(&vt)));
                                }
                                _ => {
                                    return fail(&vt, "litera dysku musi być pojedynczą literą A-Z")
                                }
                            }
                        }
                        other => {
                            return fail(
                                &t,
                                format!(
                                "nieznana właściwość '{}' (dozwolone: size, format, label, letter)",
                                other
                            ),
                            )
                        }
                    }
                }
                other => {
                    return fail(
                        &t,
                        format!("nieoczekiwane {} w bloku partycji", describe(other)),
                    )
                }
            }
        }

        let (size, size_pos) = size.ok_or_else(|| {
            DsError::new(
                name_pos.line,
                name_pos.col,
                format!(
                    "partycja '{}' nie ma 'size' (podaj np. size 100GB lub size rest)",
                    name
                ),
            )
        })?;

        let fs = fs.map(|(f, _)| f);
        let (label, label_pos) = match label {
            Some((l, p)) => (Some(l), Some(p)),
            None => (None, None),
        };
        let (letter, letter_pos) = match letter {
            Some((c, p)) => (Some(c), Some(p)),
            None => (None, None),
        };

        Ok(Partition {
            name,
            name_pos,
            size,
            size_pos,
            fs,
            label,
            label_pos,
            letter,
            letter_pos,
        })
    }
}
