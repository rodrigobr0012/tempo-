//! Persistência em SQLite e serviço de aplicação.
//!
//! [`Tempo`] é a única porta de entrada da interface: ele aplica as regras do
//! `tempo-core` e grava tudo de forma transacional.

mod error;
pub mod model;
mod schema;
mod service;

pub use error::{Error, Result};
pub use model::*;
pub use service::Tempo;
