#![allow(deprecated)]

use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::Duration;

use eframe::egui;

use crate::ast::*;
use crate::backend::{diskpart::DiskpartBackend, Backend};
use crate::export::to_dscript;
use crate::i18n::Lang;
use crate::system::{self, DiskInfo, SysError};
use crate::tr;
use crate::validator;

const RED: egui::Color32 = egui::Color32::LIGHT_RED;
const GREEN: egui::Color32 = egui::Color32::LIGHT_GREEN;

#[derive(Clone, Copy, PartialEq)]
enum Unit {
    Mb,
    Gb,
    Tb,
}

impl Unit {
    fn name(self) -> &'static str {
        match self {
            Unit::Mb => "MB",
            Unit::Gb => "GB",
            Unit::Tb => "TB",
        }
    }

    fn factor(self) -> u64 {
        match self {
            Unit::Mb => 1,
            Unit::Gb => 1024,
            Unit::Tb => 1024 * 1024,
        }
    }
}

/// Jeden wiersz w oknie = jedna partycja.
struct PartRow {
    rest: bool,
    amount: u64,
    unit: Unit,
    fs: Option<FsKind>,
    label: String,
    letter: Option<char>,
}

impl PartRow {
    fn new() -> Self {
        Self {
            rest: false,
            amount: 10,
            unit: Unit::Gb,
            fs: Some(FsKind::Ntfs),
            label: String::new(),
            letter: None,
        }
    }
}

/// Wynik ostatniej operacji – przechowywany jako dane, tłumaczony przy wyświetlaniu.
enum Outcome {
    Done(String),
    Failed(SysError),
    Saved(String),
    SaveFailed(String),
}

struct App {
    lang: Lang,
    disks: Vec<DiskInfo>,
    disk_error: Option<SysError>,
    selected: Option<usize>,
    table: TableKind,
    parts: Vec<PartRow>,
    outcome: Option<Outcome>,
    confirming: bool,
    confirm_text: String,
    job: Option<Receiver<Result<String, SysError>>>,
}

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([860.0, 780.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };
    eframe::run_native(
        "DiskScript",
        options,
        Box::new(|_cc| Ok(Box::new(App::new()))),
    )
}

impl App {
    fn new() -> Self {
        let mut app = App {
            lang: Lang::Pl,
            disks: Vec::new(),
            disk_error: None,
            selected: None,
            table: TableKind::Gpt,
            parts: vec![PartRow::new()],
            outcome: None,
            confirming: false,
            confirm_text: String::new(),
            job: None,
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        match system::list_disks() {
            Ok(d) => {
                self.disks = d;
                self.disk_error = None;
            }
            Err(e) => {
                self.disks.clear();
                self.disk_error = Some(e);
            }
        }
        let valid = self
            .selected
            .is_some_and(|i| self.disks.get(i).is_some_and(|d| !d.is_system));
        if !valid {
            self.selected = None;
        }
    }

    /// Zamienia to, co jest w oknie, na to samo drzewo AST, które tworzy parser.
    /// Numer linii w pozycjach = numer partycji (dla komunikatów walidatora).
    fn build_script(&self) -> Option<Script> {
        let disk = &self.disks[self.selected?];
        let pos = |i: usize| Pos {
            line: i + 1,
            col: 0,
        };

        let partitions: Vec<Partition> = self
            .parts
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let size = if p.rest {
                    Size::Rest
                } else {
                    Size::Mb(p.amount.saturating_mul(p.unit.factor()))
                };
                let label = if p.fs.is_some() {
                    p.label.trim().replace('"', "")
                } else {
                    String::new()
                };
                let has_label = !label.is_empty();

                Partition {
                    name: format!("partition{}", i + 1),
                    name_pos: pos(i),
                    size,
                    size_pos: pos(i),
                    fs: p.fs,
                    label: if has_label { Some(label) } else { None },
                    label_pos: if has_label { Some(pos(i)) } else { None },
                    letter: p.letter,
                    letter_pos: p.letter.map(|_| pos(i)),
                }
            })
            .collect();

        Some(Script {
            disks: vec![Disk {
                number: disk.number,
                number_pos: Pos { line: 0, col: 0 },
                pos: Pos { line: 0, col: 0 },
                table: Some(self.table),
                partitions,
            }],
        })
    }

    /// Błędy walidatora + sprawdzenia, które znamy dzięki prawdziwemu rozmiarowi dysku.
    fn problems(&self, script: &Script) -> Vec<String> {
        let lang = self.lang;
        let mut out = Vec::new();

        // Dysk systemowy jest blokowany przy wyborze, więc dysk 0 przepuszczamy.
        if let Err(errs) = validator::validate_lang(script, true, lang) {
            for e in errs {
                if e.line > 0 {
                    out.push(tr!(
                        lang,
                        "Partycja {}: {}",
                        "Partition {}: {}",
                        e.line,
                        e.message
                    ));
                } else {
                    out.push(e.message);
                }
            }
        }

        let Some(idx) = self.selected else { return out };
        let disk_mb = self.disks[idx].size_mb();
        let parts = &script.disks[0].partitions;

        let explicit: u64 = parts
            .iter()
            .filter_map(|p| match p.size {
                Size::Mb(mb) => Some(mb),
                Size::Rest => None,
            })
            .sum();
        let has_rest = parts.iter().any(|p| p.size == Size::Rest);

        if explicit > disk_mb {
            out.push(tr!(
                lang,
                "Suma rozmiarów partycji ({:.1} GB) przekracza rozmiar dysku ({:.1} GB)",
                "The total partition size ({:.1} GB) exceeds the disk size ({:.1} GB)",
                explicit as f64 / 1024.0,
                disk_mb as f64 / 1024.0
            ));
        } else if has_rest && explicit + 8 > disk_mb {
            out.push(
                lang.t(
                    "Brak miejsca na partycję 'cała reszta dysku' – pozostałe partycje zajmują cały dysk",
                    "No space left for the 'rest of the disk' partition – the other partitions fill the whole disk",
                )
                    .to_string(),
            );
        } else if has_rest {
            let remaining = disk_mb - explicit;
            for (i, p) in parts.iter().enumerate() {
                if p.size == Size::Rest && p.fs == Some(FsKind::Fat32) && remaining > 32 * 1024 {
                    out.push(tr!(
                        lang,
                        "Partycja {}: FAT32 nie obsłuży reszty dysku (ok. {:.0} GB, limit 32 GB) – wybierz exFAT lub NTFS albo podaj rozmiar",
                        "Partition {}: FAT32 cannot handle the rest of the disk (about {:.0} GB, limit 32 GB) – choose exFAT or NTFS, or set a size",
                        i + 1,
                        remaining as f64 / 1024.0
                    ));
                }
            }
        }

        out
    }

    fn poll_job(&mut self, ctx: &egui::Context) {
        let polled = self.job.as_ref().map(|rx| rx.try_recv());
        match polled {
            None => {}
            Some(Ok(result)) => {
                self.job = None;
                self.outcome = Some(match result {
                    Ok(output) => Outcome::Done(output),
                    Err(e) => Outcome::Failed(e),
                });
                self.refresh();
            }
            Some(Err(TryRecvError::Empty)) => {
                ctx.request_repaint_after(Duration::from_millis(200));
            }
            Some(Err(TryRecvError::Disconnected)) => {
                self.job = None;
            }
        }
    }

    fn start_job(&mut self, ctx: &egui::Context) {
        let Some(script) = self.build_script() else {
            return;
        };
        if !self.problems(&script).is_empty() {
            return;
        }
        let text = DiskpartBackend.generate(&script);

        let (tx, rx) = mpsc::channel();
        let ctx2 = ctx.clone();
        thread::spawn(move || {
            let result = system::run_diskpart(&text);
            let _ = tx.send(result);
            ctx2.request_repaint();
        });

        self.job = Some(rx);
        self.outcome = None;
    }

    fn save_dscript(&mut self, script: &Script) {
        let path = self.lang.t("ustawienia.dscript", "settings.dscript");
        self.outcome = Some(match std::fs::write(path, to_dscript(script)) {
            Ok(()) => {
                let dir = std::env::current_dir()
                    .map(|d| d.display().to_string())
                    .unwrap_or_default();
                Outcome::Saved(format!("{}|{}", path, dir))
            }
            Err(e) => Outcome::SaveFailed(e.to_string()),
        });
    }

    fn ui_disks(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;

        ui.horizontal(|ui| {
            ui.strong(lang.t("1. Dysk", "1. Disk"));
            if ui.button(lang.t("Odśwież listę", "Refresh list")).clicked() {
                self.refresh();
            }
        });

        if let Some(err) = &self.disk_error {
            ui.colored_label(RED, err.text(lang));
        }

        let mut new_selected = self.selected;
        for (idx, d) in self.disks.iter().enumerate() {
            let text = d.describe(lang);
            if d.is_system {
                ui.add_enabled_ui(false, |ui| {
                    let _ = ui.selectable_label(
                        false,
                        tr!(
                            lang,
                            "{}   [SYSTEMOWY – zablokowany]",
                            "{}   [SYSTEM – locked]",
                            text
                        ),
                    );
                });
            } else if ui
                .selectable_label(self.selected == Some(idx), text)
                .clicked()
            {
                new_selected = Some(idx);
            }
        }
        self.selected = new_selected;

        ui.horizontal(|ui| {
            ui.label(lang.t("Tablica partycji:", "Partition table:"));
            ui.radio_value(
                &mut self.table,
                TableKind::Gpt,
                lang.t("GPT (nowoczesna, zalecana)", "GPT (modern, recommended)"),
            );
            ui.radio_value(
                &mut self.table,
                TableKind::Mbr,
                lang.t(
                    "MBR (starsze systemy, max 4 partycje)",
                    "MBR (legacy systems, max 4 partitions)",
                ),
            );
        });
    }

    fn ui_partitions(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        ui.strong(lang.t("2. Partycje", "2. Partitions"));
        let mut remove: Option<usize> = None;

        for i in 0..self.parts.len() {
            let taken: Vec<char> = self
                .parts
                .iter()
                .enumerate()
                .filter(|(j, _)| *j != i)
                .filter_map(|(_, p)| p.letter)
                .collect();
            let p = &mut self.parts[i];

            ui.group(|ui| {
                ui.horizontal(|ui| {
                    ui.strong(tr!(lang, "Partycja {}", "Partition {}", i + 1));
                    if ui.button(lang.t("Usuń", "Remove")).clicked() {
                        remove = Some(i);
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(lang.t("Rozmiar:", "Size:"));
                    ui.checkbox(&mut p.rest, lang.t("cała reszta dysku", "rest of the disk"));
                    if !p.rest {
                        ui.add(egui::DragValue::new(&mut p.amount).speed(1.0));
                        if p.amount == 0 {
                            p.amount = 1;
                        }
                        egui::ComboBox::from_id_source(("unit", i))
                            .width(60.0)
                            .selected_text(p.unit.name())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut p.unit, Unit::Mb, "MB");
                                ui.selectable_value(&mut p.unit, Unit::Gb, "GB");
                                ui.selectable_value(&mut p.unit, Unit::Tb, "TB");
                            });
                    }
                });

                ui.horizontal(|ui| {
                    ui.label(lang.t("Format:", "Format:"));
                    let fs_text = match p.fs {
                        Some(f) => f.as_str().to_uppercase(),
                        None => lang.t("bez formatowania", "no formatting").to_string(),
                    };
                    egui::ComboBox::from_id_source(("fs", i))
                        .selected_text(fs_text)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut p.fs,
                                None,
                                lang.t("Bez formatowania (RAW)", "No formatting (RAW)"),
                            );
                            ui.selectable_value(&mut p.fs, Some(FsKind::Ntfs), "NTFS");
                            ui.selectable_value(&mut p.fs, Some(FsKind::Exfat), "exFAT");
                            ui.selectable_value(&mut p.fs, Some(FsKind::Fat32), "FAT32");
                            ui.selectable_value(&mut p.fs, Some(FsKind::Refs), "ReFS");
                        });

                    ui.label(lang.t("Etykieta:", "Label:"));
                    ui.add_enabled(
                        p.fs.is_some(),
                        egui::TextEdit::singleline(&mut p.label).desired_width(120.0),
                    );

                    ui.label(lang.t("Litera:", "Letter:"));
                    let letter_text = match p.letter {
                        Some(c) => format!("{}:", c),
                        None => "auto".to_string(),
                    };
                    egui::ComboBox::from_id_source(("letter", i))
                        .width(70.0)
                        .selected_text(letter_text)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut p.letter,
                                None,
                                lang.t("auto (Windows wybierze)", "auto (Windows picks)"),
                            );
                            for c in 'D'..='Z' {
                                if taken.contains(&c) {
                                    continue;
                                }
                                let text = if system::letter_in_use(c) {
                                    tr!(
                                        lang,
                                        "{}: (zajęta w systemie)",
                                        "{}: (in use by the system)",
                                        c
                                    )
                                } else {
                                    format!("{}:", c)
                                };
                                ui.selectable_value(&mut p.letter, Some(c), text);
                            }
                        });
                });
            });
        }

        if let Some(i) = remove {
            self.parts.remove(i);
        }
        if ui
            .button(lang.t("+ Dodaj partycję", "+ Add partition"))
            .clicked()
        {
            self.parts.push(PartRow::new());
        }
    }

    fn ui_summary(&mut self, ui: &mut egui::Ui, busy: bool) {
        let lang = self.lang;
        ui.strong(lang.t("3. Podsumowanie", "3. Summary"));

        let Some(script) = self.build_script() else {
            ui.label(lang.t(
                "Wybierz dysk (krok 1), aby zobaczyć podsumowanie.",
                "Select a disk (step 1) to see the summary.",
            ));
            return;
        };
        let problems = self.problems(&script);

        if problems.is_empty() {
            ui.colored_label(
                GREEN,
                lang.t("Konfiguracja poprawna.", "Configuration is valid."),
            );
        } else {
            for p in &problems {
                ui.colored_label(RED, format!("• {}", p));
            }
        }

        ui.add_space(4.0);
        ui.label(lang.t(
            "Podgląd skryptu diskpart (nic nie jest jeszcze uruchomione):",
            "diskpart script preview (nothing has been run yet):",
        ));
        let preview = DiskpartBackend.generate(&script);
        egui::ScrollArea::vertical()
            .max_height(180.0)
            .show(ui, |ui| {
                ui.monospace(preview);
            });

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let can_run = !busy && problems.is_empty();
            if ui
                .add_enabled(
                    can_run,
                    egui::Button::new(lang.t("Formatuj...", "Format...")),
                )
                .clicked()
            {
                self.confirming = true;
                self.confirm_text.clear();
            }
            if ui
                .button(lang.t("Zapisz jako .dscript", "Save as .dscript"))
                .clicked()
            {
                self.save_dscript(&script);
            }
        });
    }

    fn ui_main(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading("DiskScript");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.selectable_value(&mut self.lang, Lang::En, "English");
                ui.selectable_value(&mut self.lang, Lang::Pl, "Polski");
            });
        });

        let lang = self.lang;
        ui.label(lang.t(
            "Wybierz dysk, ułóż partycje i formaty. Nic się nie zmieni, dopóki nie klikniesz 'Formatuj' i nie potwierdzisz.",
            "Pick a disk, lay out partitions and formats. Nothing changes until you click 'Format' and confirm.",
        ));
        ui.add_space(6.0);

        let busy = self.job.is_some();
        ui.add_enabled_ui(!busy, |ui| {
            self.ui_disks(ui);
            ui.add_space(8.0);
            self.ui_partitions(ui);
        });

        ui.separator();
        self.ui_summary(ui, busy);

        // Status ostatniej operacji (tłumaczony na bieżąco).
        let (status, log): (Option<String>, Option<String>) = if busy {
            (
                Some(
                    lang.t(
                        "Trwa formatowanie… nie zamykaj programu.",
                        "Formatting in progress… do not close the program.",
                    )
                    .to_string(),
                ),
                None,
            )
        } else {
            match &self.outcome {
                None => (None, None),
                Some(Outcome::Done(out)) => (
                    Some(
                        lang.t(
                            "Gotowe – skrypt diskpart wykonany. Sprawdź wynik poniżej.",
                            "Done – the diskpart script finished. See the output below.",
                        )
                        .to_string(),
                    ),
                    Some(out.clone()),
                ),
                Some(Outcome::Failed(e)) => (
                    Some(
                        lang.t(
                            "Błąd podczas formatowania – szczegóły poniżej.",
                            "Error while formatting – details below.",
                        )
                        .to_string(),
                    ),
                    Some(e.text(lang)),
                ),
                Some(Outcome::Saved(info)) => {
                    let (file, dir) = info.split_once('|').unwrap_or((info.as_str(), ""));
                    (
                        Some(tr!(
                            lang,
                            "Zapisano {} w folderze {}",
                            "Saved {} in folder {}",
                            file,
                            dir
                        )),
                        None,
                    )
                }
                Some(Outcome::SaveFailed(e)) => (
                    Some(tr!(
                        lang,
                        "Nie udało się zapisać pliku: {}",
                        "Could not save the file: {}",
                        e
                    )),
                    None,
                ),
            }
        };

        if let Some(s) = status {
            ui.add_space(6.0);
            ui.label(s);
        }
        if let Some(l) = log {
            ui.collapsing(lang.t("Wynik diskpart", "diskpart output"), |ui| {
                egui::ScrollArea::vertical()
                    .max_height(200.0)
                    .show(ui, |ui| {
                        ui.monospace(l);
                    });
            });
        }
    }

    fn ui_confirm(&mut self, ctx: &egui::Context) {
        if !self.confirming {
            return;
        }
        let Some(idx) = self.selected else {
            self.confirming = false;
            return;
        };
        let lang = self.lang;
        let disk = self.disks[idx].clone();
        let word = lang.t("SKASUJ", "ERASE");
        let mut start = false;
        let mut cancel = false;

        egui::Window::new(lang.t(
            "Potwierdzenie – operacji nie da się cofnąć",
            "Confirmation – this cannot be undone",
        ))
        .id(egui::Id::new("confirm_window"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.colored_label(
                RED,
                lang.t(
                    "Wszystkie dane na tym dysku zostaną bezpowrotnie skasowane:",
                    "All data on this disk will be permanently erased:",
                ),
            );
            ui.strong(disk.describe(lang));
            ui.add_space(6.0);
            ui.label(tr!(
                lang,
                "Wpisz {}, aby kontynuować:",
                "Type {} to continue:",
                word
            ));
            ui.text_edit_singleline(&mut self.confirm_text);
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                let ok = self.confirm_text.trim() == word;
                if ui
                    .add_enabled(ok, egui::Button::new(lang.t("Formatuj", "Format")))
                    .clicked()
                {
                    start = true;
                }
                if ui.button(lang.t("Anuluj", "Cancel")).clicked() {
                    cancel = true;
                }
            });
        });

        if cancel {
            self.confirming = false;
        }
        if start {
            self.confirming = false;
            self.start_job(ctx);
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_job(ctx);

        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    self.ui_main(ui);
                });
        });

        self.ui_confirm(ctx);
    }
}
