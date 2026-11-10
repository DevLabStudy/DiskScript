//! Drzewo składniowe (AST) – to, co parser buduje z tokenów.

/// Pozycja w pliku źródłowym (numerowana od 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pos {
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TableKind {
    Gpt,
    Mbr,
}

impl TableKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TableKind::Gpt => "gpt",
            TableKind::Mbr => "mbr",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FsKind {
    Ntfs,
    Fat32,
    Exfat,
    Refs,
}

impl FsKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            FsKind::Ntfs => "ntfs",
            FsKind::Fat32 => "fat32",
            FsKind::Exfat => "exfat",
            FsKind::Refs => "refs",
        }
    }

    /// Maksymalna długość etykiety (znaki) dla danego systemu plików.
    pub fn max_label_len(&self) -> usize {
        match self {
            FsKind::Ntfs | FsKind::Refs => 32,
            FsKind::Fat32 | FsKind::Exfat => 11,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Size {
    /// Rozmiar w megabajtach (tego oczekuje diskpart).
    Mb(u64),
    /// Całe pozostałe miejsce.
    Rest,
}

#[derive(Debug, Clone)]
pub struct Partition {
    pub name: String,
    pub name_pos: Pos,
    pub size: Size,
    pub size_pos: Pos,
    pub fs: Option<FsKind>,
    pub label: Option<String>,
    pub label_pos: Option<Pos>,
    pub letter: Option<char>,
    pub letter_pos: Option<Pos>,
}

#[derive(Debug, Clone)]
pub struct Disk {
    pub number: u32,
    /// Pozycja numeru dysku (np. cyfra w `disk 1`).
    pub number_pos: Pos,
    /// Pozycja słowa kluczowego `disk`.
    pub pos: Pos,
    pub table: Option<TableKind>,
    pub partitions: Vec<Partition>,
}

#[derive(Debug, Clone)]
pub struct Script {
    pub disks: Vec<Disk>,
}
