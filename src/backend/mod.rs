pub mod diskpart;

use crate::ast::Script;

/// Backend zamienia AST na konkretne polecenia.
/// Dziś jest jeden (diskpart); później można dodać np. Linuksa (parted + mkfs).
pub trait Backend {
    fn generate(&self, script: &Script) -> String;
}
