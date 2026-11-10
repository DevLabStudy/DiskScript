/// Języki interfejsu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Lang {
    Pl,
    En,
}

impl Lang {
    /// Wybiera tekst zależnie od języka: lang.t("po polsku", "in English")
    pub fn t(self, pl: &'static str, en: &'static str) -> &'static str {
        match self {
            Lang::Pl => pl,
            Lang::En => en,
        }
    }
}

/// Tekst z argumentami: tr!(lang, "dysk {}", "disk {}", numer) -> String
#[macro_export]
macro_rules! tr {
    ($lang:expr, $pl:literal, $en:literal $(, $arg:expr)* $(,)?) => {
        match $lang {
            $crate::i18n::Lang::Pl => format!($pl $(, $arg)*),
            $crate::i18n::Lang::En => format!($en $(, $arg)*),
        }
    };
}
