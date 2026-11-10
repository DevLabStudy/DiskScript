mod ast;
mod backend;
mod error;
mod export;
mod gui;
mod i18n;
mod lexer;
mod parser;
mod system;
mod validator;

use std::io::{self, Write};
use std::{env, fs, process};

use backend::{diskpart::DiskpartBackend, Backend};
use error::DsError;

const HELP: &str = "\
DiskScript – czytelny język i okno do partycjonowania i formatowania dysków

UŻYCIE:
    diskscript                      otwiera okno graficzne
    diskscript --gui                to samo
    diskscript <plik.dscript> [opcje]

OPCJE (tryb pliku):
    --check          tylko sprawdź skrypt (nic nie wypisuj ani nie uruchamiaj)
    --out <plik>     zapisz wygenerowany skrypt diskpart do pliku
    --run            wykonaj skrypt przez diskpart (tylko Windows, jako administrator)
    --allow-disk0    pozwól na dysk 0 (domyślnie zablokowane)
    -h, --help       ta pomoc

Domyślnie tryb pliku działa jako dry-run: wypisuje wygenerowany skrypt i niczego nie uruchamia.";

struct Args {
    path: String,
    check: bool,
    run: bool,
    out: Option<String>,
    allow_disk0: bool,
}

fn parse_args() -> Result<Args, String> {
    let mut path = None;
    let (mut check, mut run, mut allow_disk0) = (false, false, false);
    let mut out = None;

    let mut it = env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "-h" | "--help" => {
                println!("{}", HELP);
                process::exit(0);
            }
            "--check" => check = true,
            "--run" => run = true,
            "--allow-disk0" => allow_disk0 = true,
            "--out" => out = Some(it.next().ok_or("po --out podaj nazwę pliku")?),
            s if s.starts_with('-') => return Err(format!("nieznana opcja '{}'", s)),
            s => {
                if path.is_some() {
                    return Err("podano więcej niż jeden plik".to_string());
                }
                path = Some(s.to_string());
            }
        }
    }

    Ok(Args {
        path: path.ok_or("podaj plik .dscript (diskscript --help)")?,
        check,
        run,
        out,
        allow_disk0,
    })
}

/// Cały potok: tekst -> tokeny -> AST -> walidacja.
fn compile(src: &str, allow_disk0: bool) -> Result<ast::Script, Vec<DsError>> {
    let tokens = lexer::Lexer::new(src).tokenize().map_err(|e| vec![e])?;
    let script = parser::Parser::new(tokens)
        .parse_script()
        .map_err(|e| vec![e])?;
    validator::validate(&script, allow_disk0)?;
    Ok(script)
}

fn confirm(script: &ast::Script) -> bool {
    println!("UWAGA: poniższe dyski zostaną CAŁKOWICIE WYCZYSZCZONE (dane przepadną):");
    for d in &script.disks {
        println!("  - dysk {}", d.number);
    }
    print!("Wpisz dokładnie: TAK, SKASUJ > ");
    let _ = io::stdout().flush();

    let mut input = String::new();
    io::stdin().read_line(&mut input).is_ok() && input.trim() == "TAK, SKASUJ"
}

fn main() {
    let argv: Vec<String> = env::args().skip(1).collect();
    if argv.first().map(String::as_str) == Some("--elevated-run") {
        #[cfg(windows)]
        {
            if argv.len() != 3 {
                process::exit(2);
            }
            process::exit(system::run_elevated_child(&argv[1], &argv[2]));
        }
        #[cfg(not(windows))]
        process::exit(1);
    }
    if argv.is_empty() || argv.iter().any(|a| a == "--gui") {
        if let Err(e) = gui::run() {
            eprintln!("błąd okna: {}", e);
            process::exit(1);
        }
        return;
    }

    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("błąd: {}\n\n{}", e, HELP);
            process::exit(2);
        }
    };

    let source = match fs::read_to_string(&args.path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("nie można odczytać '{}': {}", args.path, e);
            process::exit(2);
        }
    };

    let script = match compile(&source, args.allow_disk0) {
        Ok(s) => s,
        Err(errors) => {
            for e in &errors {
                eprintln!("{}", e.render(&source));
            }
            process::exit(1);
        }
    };

    if args.check {
        println!("OK: skrypt jest poprawny ({} dysk/ów).", script.disks.len());
        return;
    }

    let output = DiskpartBackend.generate(&script);

    if let Some(path) = &args.out {
        if let Err(e) = fs::write(path, &output) {
            eprintln!("nie można zapisać '{}': {}", path, e);
            process::exit(2);
        }
        println!("Zapisano skrypt diskpart do '{}'.", path);
    }

    if args.run {
        if !confirm(&script) {
            println!("Przerwano – nic nie zostało zmienione.");
            return;
        }
        match system::run_diskpart(&output) {
            Ok(text) => println!("{}", text),
            Err(e) => {
                eprintln!("błąd: {}", e.text(i18n::Lang::Pl));
                process::exit(1);
            }
        }
    } else if args.out.is_none() {
        println!("--- DRY-RUN: tak wyglądałby skrypt diskpart (nic nie uruchomiono) ---\n");
        print!("{}", output);
    }
}
