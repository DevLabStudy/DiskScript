use super::Backend;
use crate::ast::*;

pub struct DiskpartBackend;

impl Backend for DiskpartBackend {
    fn generate(&self, script: &Script) -> String {
        let mut out = String::new();

        for disk in &script.disks {
            let table = disk.table.unwrap_or(TableKind::Gpt);

            out.push_str(&format!("rem === disk {} ===\n", disk.number));
            out.push_str(&format!("select disk {}\n", disk.number));
            out.push_str("clean\n");
            out.push_str(&format!("convert {}\n", table.as_str()));

            for p in &disk.partitions {
                out.push_str(&format!("rem partition {}\n", p.name));

                match p.size {
                    Size::Mb(mb) => {
                        out.push_str(&format!("create partition primary size={}\n", mb))
                    }
                    Size::Rest => out.push_str("create partition primary\n"),
                }

                if let Some(fs) = p.fs {
                    let mut line = format!("format fs={} quick", fs.as_str());
                    if let Some(label) = &p.label {
                        line.push_str(&format!(" label=\"{}\"", label));
                    }
                    out.push_str(&line);
                    out.push('\n');
                }

                if let Some(letter) = p.letter {
                    out.push_str(&format!("assign letter={}\n", letter));
                }
            }
            out.push('\n');
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{lexer::Lexer, parser::Parser, validator};

    fn compile(src: &str) -> Script {
        let tokens = Lexer::new(src).tokenize().unwrap();
        let script = Parser::new(tokens).parse_script().unwrap();
        validator::validate(&script, false).unwrap();
        script
    }

    #[test]
    fn generuje_oczekiwany_skrypt() {
        let script = compile(
            r#"disk 1 {
                table gpt
                partition dane { size 100GB format ntfs label "Dane" letter e }
                partition reszta { size rest format exfat }
            }"#,
        );
        let out = DiskpartBackend.generate(&script);
        assert!(out.contains("select disk 1\nclean\nconvert gpt\n"));
        assert!(out.contains("create partition primary size=102400\n"));
        assert!(out.contains("format fs=ntfs quick label=\"Dane\"\n"));
        assert!(out.contains("assign letter=E\n"));
        assert!(out.contains("create partition primary\nformat fs=exfat quick\n"));
    }

    #[test]
    fn odrzuca_dysk_zero() {
        let tokens = Lexer::new("disk 0 { partition a { size rest } }")
            .tokenize()
            .unwrap();
        let script = Parser::new(tokens).parse_script().unwrap();
        assert!(validator::validate(&script, false).is_err());
    }

    #[test]
    fn odrzuca_powtorzona_litere() {
        let tokens = Lexer::new(
            "disk 1 { partition a { size 1GB letter E } partition b { size rest letter E } }",
        )
        .tokenize()
        .unwrap();
        let script = Parser::new(tokens).parse_script().unwrap();
        assert!(validator::validate(&script, false).is_err());
    }
}
