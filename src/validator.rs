use std::collections::{HashMap, HashSet};

use crate::ast::*;
use crate::error::DsError;
use crate::i18n::Lang;
use crate::tr;

fn err(pos: Pos, msg: impl Into<String>) -> DsError {
    DsError::new(pos.line, pos.col, msg)
}

/// Wersja polska (używana przez tryb pliku i testy).
pub fn validate(script: &Script, allow_disk0: bool) -> Result<(), Vec<DsError>> {
    validate_lang(script, allow_disk0, Lang::Pl)
}

/// Sprawdza sens skryptu ZANIM cokolwiek zostanie wygenerowane lub uruchomione.
/// Zwraca wszystkie znalezione błędy naraz, każdy ze wskazaniem konkretnego miejsca.
pub fn validate_lang(script: &Script, allow_disk0: bool, lang: Lang) -> Result<(), Vec<DsError>> {
    let mut errs: Vec<DsError> = Vec::new();
    let mut seen_disks = HashSet::new();
    let mut seen_letters: HashMap<char, Pos> = HashMap::new();

    if script.disks.is_empty() {
        errs.push(DsError::new(
            1,
            0,
            lang.t(
                "skrypt jest pusty – brak bloku 'disk'",
                "the script is empty – no 'disk' block",
            ),
        ));
    }

    for disk in &script.disks {
        if disk.number == 0 && !allow_disk0 {
            errs.push(err(
                disk.number_pos,
                lang.t(
                    "dysk 0 to zwykle dysk systemowy – odmowa (użyj --allow-disk0, jeśli wiesz, co robisz)",
                    "disk 0 is usually the system disk – refusing (use --allow-disk0 if you know what you are doing)",
                ),
            ));
        }
        if !seen_disks.insert(disk.number) {
            errs.push(err(
                disk.number_pos,
                tr!(
                    lang,
                    "dysk {} opisano więcej niż raz",
                    "disk {} is described more than once",
                    disk.number
                ),
            ));
        }
        if disk.partitions.is_empty() {
            errs.push(err(
                disk.pos,
                tr!(
                    lang,
                    "dysk {} nie ma żadnej partycji",
                    "disk {} has no partitions",
                    disk.number
                ),
            ));
        }

        let table = disk.table.unwrap_or(TableKind::Gpt);
        if table == TableKind::Mbr && disk.partitions.len() > 4 {
            errs.push(err(
                disk.pos,
                lang.t(
                    "tablica MBR pozwala na maksymalnie 4 partycje główne",
                    "an MBR table allows at most 4 primary partitions",
                ),
            ));
        }

        let mut names = HashSet::new();
        let last = disk.partitions.len().saturating_sub(1);

        for (i, p) in disk.partitions.iter().enumerate() {
            if !names.insert(p.name.clone()) {
                errs.push(err(
                    p.name_pos,
                    tr!(
                        lang,
                        "nazwa partycji '{}' powtarza się na dysku {}",
                        "partition name '{}' is used more than once on disk {}",
                        p.name,
                        disk.number
                    ),
                ));
            }

            if p.size == Size::Rest && i != last {
                errs.push(err(
                    p.size_pos,
                    lang.t(
                        "'size rest' może mieć tylko ostatnia partycja na dysku",
                        "only the last partition on a disk can use 'size rest'",
                    ),
                ));
            }

            if let Some(l) = p.letter {
                let lpos = p.letter_pos.unwrap_or(p.name_pos);
                if l == 'C' {
                    errs.push(err(
                        lpos,
                        lang.t("litera C jest zarezerwowana", "letter C is reserved"),
                    ));
                }
                if let Some(prev) = seen_letters.insert(l, lpos) {
                    errs.push(err(
                        lpos,
                        tr!(
                            lang,
                            "litera {} jest już użyta (linia {})",
                            "letter {} is already used (line {})",
                            l,
                            prev.line
                        ),
                    ));
                }
            }

            if let Some(label) = &p.label {
                let lpos = p.label_pos.unwrap_or(p.name_pos);
                match p.fs {
                    None => errs.push(err(
                        lpos,
                        lang.t(
                            "'label' wymaga 'format' (etykietę nadaje się przy formatowaniu)",
                            "'label' requires 'format' (the label is set while formatting)",
                        ),
                    )),
                    Some(fs) => {
                        let max = fs.max_label_len();
                        if label.chars().count() > max {
                            errs.push(err(
                                lpos,
                                tr!(
                                    lang,
                                    "etykieta jest za długa dla {} (max {} znaków)",
                                    "the label is too long for {} (max {} characters)",
                                    fs.as_str(),
                                    max
                                ),
                            ));
                        }
                    }
                }
            }

            if p.fs == Some(FsKind::Fat32) {
                // Dla 'rest' nie znamy rozmiaru dysku, więc sprawdzamy tylko jawne rozmiary.
                if let Size::Mb(mb) = p.size {
                    if mb > 32 * 1024 {
                        errs.push(err(
                            p.size_pos,
                            lang.t(
                                "diskpart nie sformatuje FAT32 większego niż 32GB – zmniejsz rozmiar albo użyj ntfs lub exfat",
                                "diskpart cannot format FAT32 larger than 32GB – reduce the size or use ntfs or exfat",
                            ),
                        ));
                    }
                }
            }
        }
    }

    if errs.is_empty() {
        Ok(())
    } else {
        Err(errs)
    }
}
