use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum AudioError {
    #[error("сеть: {0}")]
    Network(String),
    #[error("файл: {0}")]
    Io(String),
    #[error("формат не распознан: {0}")]
    Format(String),
    #[error("кодек не поддерживается: {0}")]
    Codec(String),
    #[error("декодирование: {0}")]
    Decode(String),
    #[error("вывод звука: {0}")]
    Output(String),
}

impl From<std::io::Error> for AudioError {
    fn from(e: std::io::Error) -> Self {
        AudioError::Io(e.to_string())
    }
}
