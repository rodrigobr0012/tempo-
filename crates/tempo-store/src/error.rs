use tempo_core::CoreError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    Rule(#[from] CoreError),

    #[error("Erro ao acessar os dados: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("Tag não encontrada.")]
    TagNotFound,

    #[error("Sessão não encontrada.")]
    SessionNotFound,

    #[error("Pare o timer antes de apagar a sessão em andamento.")]
    SessionActive,

    #[error("Só é possível apagar sessões do dia atual.")]
    SessionNotToday,

    #[error("Configure o sono antes de continuar.")]
    NotOnboarded,

    #[error("O tempo de \"{0}\" acabou hoje. Ative o excedente nos ajustes para continuar.")]
    TimeExhausted(String),

    #[error("Há espaço para restaurar \"{0}\" apenas reduzindo outras tags.")]
    NoRoomToRestore(String),
}

pub type Result<T> = std::result::Result<T, Error>;
