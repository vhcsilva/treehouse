//! `treehouse-agent`: roda dentro do WSL e faz tudo que toca arquivos, git e
//! processos para o app Treehouse.

pub mod daemon;
pub mod error;
pub mod git;
pub mod paths;
pub mod server;
pub mod term;
