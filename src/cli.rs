use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "maxtube",
    version,
    about = "Gerencia titulos do YouTube com sugestoes de IA local (Ollama)"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Faz login OAuth na sua conta do YouTube (abre o navegador)
    Auth,
    /// Abre um painel web local para colar as credenciais e fazer login
    Panel,
    /// Baixa os videos do canal para data/videos.json
    Pull {
        /// Limite de videos a puxar (padrao: todos)
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Gera sugestoes de titulos com o Ollama
    Generate {
        /// Modelo Ollama (sobrepoe o config.toml)
        #[arg(long)]
        model: Option<String>,
        /// Refaz sugestoes ja existentes
        #[arg(long)]
        redo: bool,
    },
    /// Revisao interativa das sugestoes
    Review,
    /// Aplica no YouTube os titulos aprovados
    Apply {
        /// Nao pedir confirmacao
        #[arg(long)]
        yes: bool,
    },
    /// Fluxo completo: pull -> generate -> review -> apply
    Run {
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long)]
        redo: bool,
    },
    /// Mostra o status atual do projeto
    Status,
}
