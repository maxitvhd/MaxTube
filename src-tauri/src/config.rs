use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn default_model() -> String {
    "qwen3.5:4b".to_string()
}
fn default_ollama_url() -> String {
    "http://192.168.1.23:11434".to_string()
}

/// Configuracao persistida em disco (somente o essencial).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DiskConfig {
    #[serde(default = "default_model")]
    model: String,
    #[serde(default = "default_ollama_url")]
    ollama_url: String,
}

impl Default for DiskConfig {
    fn default() -> Self {
        DiskConfig {
            model: default_model(),
            ollama_url: default_ollama_url(),
        }
    }
}

/// Configuracao em memoria, com os caminhos absolutos ja resolvidos.
#[derive(Debug, Clone)]
pub struct Config {
    pub model: String,
    pub ollama_url: String,
    pub client_secret: String,
    pub token: String,
    pub data_dir: String,
    pub prompt_file: String,
    pub base: PathBuf,
}

pub const DEFAULT_PROMPT: &str = r#"Voce e um especialista em SEO e Estrategista de Conteudo no YouTube, focado no nicho de
pregacoes, estudos biblicos e cultos cristaos.

Sua missao e reescrever e otimizar o titulo de um video longo ou corte (Short) para o canal
de uma igreja, garantindo maxima Taxa de Clique (CTR) no celular e alto alcance nas buscas (SEO).

### REGRAS OBRIGATORIAS DE FORMATACAO:
1. PRIMEIROS 45 CARACTERES: a frase de maior impacto emocional, pergunta provocativa ou a
   palavra-chave mais buscada DEVE vir logo no inicio do titulo.
2. REMOVA "Tema:" E DATAS: nunca use a palavra "Tema:" nem inclua datas no titulo.
3. CAIXA ALTA E EMOJI: a frase principal deve estar em CAIXA ALTA, seguida de 1 emoji
   relevante (ex: emojis de fogo, flor, ampulheta, broto, curativo, choro).
4. GANCHOS ENTRE PARENTESES: use um gancho complementar de retencao entre parenteses, como
   "(Mensagem Forte)", "(Voce precisa ouvir)", "(Nao desista)", "(Autoavaliacao)".
5. POSICAO DO PREGADOR/EVENTO: o nome do pregador (com abreviacao: Pr., Miss., Pb., Bp.) ou o
   nome do evento deve ficar SEMPRE no final, apos " | ".
6. TAMANHO MAXIMO: o titulo total NAO deve passar de 70 a 80 caracteres.

### ESTRUTURA PADRAO A SEGUIR:
[FRASE/PERGUNTA DE IMPACTO EM CAIXA ALTA] [EMOJI] [GANCHO ENTRE PARENTESES] | [PREGADOR OU EVENTO]

### DADOS DO VIDEO (o titulo original pode conter "Tema:", data, pregador e tipo de culto):
Titulo original: {{title}}
Descricao: {{description}}

Se a descricao (ou o titulo) trouxer o nome do pregador, o tipo de culto/evento (Culto de
Missoes, Ceia, Curso DNA etc.) ou o texto biblico base, use essas informacoes.

### SAIDA:
Responda APENAS com o TITULO FINAL otimizado, em UMA UNICA LINHA, seguindo a estrutura padrao.
Sem aspas, sem explicacoes, sem listas e sem secoes numeradas. No maximo 80 caracteres.
"#;

fn base_dir() -> PathBuf {
    if let Ok(p) = std::env::var("MAXTUBE_HOME") {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".maxtube")
}

impl Config {
    pub fn load() -> Result<Config> {
        let base = base_dir();
        fs::create_dir_all(&base).with_context(|| format!("nao consegui criar {}", base.display()))?;
        let file = base.join("config.toml");

        let disk: DiskConfig = if file.exists() {
            let raw = fs::read_to_string(&file).unwrap_or_default();
            toml::from_str(&raw).unwrap_or_default()
        } else {
            let d = DiskConfig::default();
            if let Ok(raw) = toml::to_string_pretty(&d) {
                let _ = fs::write(&file, raw);
            }
            d
        };

        let cfg = Config {
            model: disk.model,
            ollama_url: disk.ollama_url,
            client_secret: base.join("client_secret.json").to_string_lossy().to_string(),
            token: base.join("token.json").to_string_lossy().to_string(),
            data_dir: base.join("data").to_string_lossy().to_string(),
            prompt_file: base.join("prompt.md").to_string_lossy().to_string(),
            base,
        };
        let _ = fs::create_dir_all(&cfg.data_dir);
        cfg.migrate_dev_secrets();
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let disk = DiskConfig {
            model: self.model.clone(),
            ollama_url: self.ollama_url.clone(),
        };
        let raw = toml::to_string_pretty(&disk)?;
        fs::write(self.base.join("config.toml"), raw).context("nao consegui salvar config.toml")?;
        Ok(())
    }

    pub fn data_path(&self, name: &str) -> PathBuf {
        PathBuf::from(&self.data_dir).join(name)
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.data_dir).ok();
        Ok(())
    }

    pub fn read_prompt(&self) -> Result<String> {
        let path = PathBuf::from(&self.prompt_file);
        if !path.exists() {
            fs::write(&path, DEFAULT_PROMPT)
                .with_context(|| format!("nao consegui criar {}", self.prompt_file))?;
        }
        fs::read_to_string(&path).with_context(|| format!("nao consegui ler {}", self.prompt_file))
    }

    pub fn write_prompt(&self, text: &str) -> Result<()> {
        fs::write(&self.prompt_file, text).context("nao consegui salvar o prompt")?;
        Ok(())
    }

    /// Se os segredos ainda nao existem em ~/.maxtube, tenta copiar de uma
    /// instalacao antiga (./.secrets ou ../.secrets) para nao precisar relogar.
    fn migrate_dev_secrets(&self) {
        let cid = PathBuf::from(&self.client_secret);
        let tok = PathBuf::from(&self.token);
        if cid.exists() && tok.exists() {
            return;
        }
        for base in [PathBuf::from(".secrets"), PathBuf::from("..").join(".secrets")] {
            if !cid.exists() {
                let src = base.join("client_secret.json");
                if src.exists() {
                    let _ = fs::copy(&src, &cid);
                }
            }
            if !tok.exists() {
                let src = base.join("token.json");
                if src.exists() {
                    let _ = fs::copy(&src, &tok);
                }
            }
        }
    }
}
