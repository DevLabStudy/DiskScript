use crate::ast::*;

fn size_text(size: Size) -> String {
    match size {
        Size::Rest => "rest".to_string(),
        Size::Mb(mb) if mb % (1024 * 1024) == 0 => format!("{}TB", mb / (1024 * 1024)),
        Size::Mb(mb) if mb % 1024 == 0 => format!("{}GB", mb / 1024),
        Size::Mb(mb) => format!("{}MB", mb),
    }
}

/// Generuje tekst w języku DiskScript z drzewa AST.
pub fn to_dscript(script: &Script) -> String {
    let mut out = String::new();

    for disk in &script.disks {
        out.push_str(&format!("disk {} {{\n", disk.number));
        if let Some(t) = disk.table {
            out.push_str(&format!("    table {}\n", t.as_str()));
        }

        for p in &disk.partitions {
            out.push_str(&format!("\n    partition {} {{\n", p.name));
            out.push_str(&format!("        size {}\n", size_text(p.size)));
            if let Some(fs) = p.fs {
                out.push_str(&format!("        format {}\n", fs.as_str()));
            }
            if let Some(label) = &p.label {
                out.push_str(&format!("        label \"{}\"\n", label));
            }
            if let Some(letter) = p.letter {
                out.push_str(&format!("        letter {}\n", letter));
            }
            out.push_str("    }\n");
        }

        out.push_str("}\n\n");
    }

    out
}
