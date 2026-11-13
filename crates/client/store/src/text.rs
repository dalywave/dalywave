//! Текст для поиска — общий с сервером (crates/textmatch): нормализация, латиница и ключ звучания.

use dalywave_textmatch::Name;
pub use dalywave_textmatch::normalize;

/// Колонки поиска строки: нормализованный текст, латиница и ключ звучания.
pub(crate) fn search_columns(s: &str) -> (String, String, String) {
    let n = Name::new(s);
    (normalize(s), n.latin().to_string(), n.sound().to_string())
}
