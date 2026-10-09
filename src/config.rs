use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

fn default_model() -> String {
    "qwen2.5:7b".to_string()
}
fn default_ollama_url() -> String {
    "http://127.0.0.1:11434".to_string()
}
fn default_prompt_file() -> String {
    "prompt.md".to_string()
}
fn default_client_secret() -> String {
    ".secrets/client_secret.json".to_string()
}
fn default_token() -> String {
    ".secrets/token.json".to_string()
}
fn default_data_dir() -> String {
    "data".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,
    #[serde(default = "default_prompt_file")]
    pub prompt_file: String,
    #[serde(default = "default_client_secret")]
    pub client_secret: String,
    #[serde(default = "default_token")]
    pub token: String,
    #[serde(default = "default_data_dir")]
    pub data_dir: String,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            model: default_model(),
            ollama_url: default_ollama_url(),
            prompt_file: default_prompt_file(),
            client_secret: default_client_secret(),
            token: default_token(),
            data_dir: default_data_dir(),
        }
    }
}

impl Config {
    pub const FILE: &'static str = "config.toml";

    /// Carrega config.toml. Se nao existir, cria a partir dos defaults.
    pub fn load() -> Result<Config> {
        let path = Path::new(Self::FILE);
        if !path.exists() {
            let cfg = Config::default();
            cfg.save()?;
            return Ok(cfg);
        }
        let raw = fs::read_to_string(path)
            .with_context(|| format!("nao consegui ler {}", Self::FILE))?;
        let cfg: Config = toml::from_str(&raw)
            .with_context(|| format!("erro de sintaxe em {}", Self::FILE))?;
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let raw = toml::to_string_pretty(self)?;
        fs::write(Self::FILE, raw).with_context(|| format!("nao consegui escrever {}", Self::FILE))?;
        Ok(())
    }

    pub fn data_path(&self, name: &str) -> PathBuf {
        PathBuf::from(&self.data_dir).join(name)
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.data_dir)
            .with_context(|| format!("nao consegui criar pasta {}", self.data_dir))?;
        if let Some(parent) = Path::new(&self.client_secret).parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).ok();
            }
        }
        Ok(())
    }

    /// Le o arquivo de prompt. Cria um template se nao existir.
    pub fn read_prompt(&self) -> Result<String> {
        let path = Path::new(&self.prompt_file);
        if !path.exists() {
            fs::write(path, DEFAULT_PROMPT)
                .with_context(|| format!("nao consegui criar {}", self.prompt_file))?;
        }
        fs::read_to_string(path)
            .with_context(|| format!("nao consegui ler {}", self.prompt_file))
    }
}

pub const DEFAULT_PROMPT: &str = r#"# Prompt principal (edite a vontade)
# Placeholders disponiveis: {{title}} e {{description}}
# Regra: o modelo deve responder APENAS com o novo titulo, em uma unica linha.

Voce e um especialista em titulos virais para YouTube.
Reescreva o titulo abaixo para ser mais atrativo e clicavel, mantendo o sentido do video,
sem clickbait enganoso e com no maximo 100 caracteres.

Titulo atual: {{title}}
Descricao: {{description}}

Responda somente com o novo titulo, sem aspas, sem explicacoes.
"#;
