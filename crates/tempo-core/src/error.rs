use thiserror::Error;

/// Erros de regra de negócio. As mensagens são exibidas diretamente ao usuário (pt-BR).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    #[error("Horário inválido: use HH:MM entre 00:00 e 23:59.")]
    InvalidClockTime,

    #[error("Os blocos reservados podem somar no máximo 23h. Restam apenas {available} para reservar.")]
    ReservedExceeded { available: String },

    #[error("Limite de 20 blocos reservados atingido.")]
    TooManyReserved,

    #[error("Porcentagem inválida: use um valor entre 0,1% e 100%.")]
    InvalidPercent,

    #[error("A soma das porcentagens ultrapassa 100%. Restam apenas {available}.")]
    AllocationExceeded { available: String },

    #[error("O nome é obrigatório.")]
    NameEmpty,

    #[error("O nome pode ter no máximo 30 caracteres.")]
    NameTooLong,

    #[error("Já existe um item chamado \"{0}\".")]
    NameDuplicate(String),

    #[error("Limite de 20 tags ativas atingido.")]
    TooManyTags,

    #[error("A duração precisa ser maior que zero.")]
    DurationZero,

    #[error("O lançamento não pode terminar no futuro.")]
    ManualInFuture,

    #[error("O lançamento precisa estar dentro do dia atual.")]
    ManualOutsideDay,

    #[error("O lançamento se sobrepõe a outra sessão já registrada.")]
    ManualOverlap,
}

pub type CoreResult<T> = Result<T, CoreError>;
