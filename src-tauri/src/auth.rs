use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::config::Config;

const AUTH_URI: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URI: &str = "https://oauth2.googleapis.com/token";
const SCOPE: &str = "https://www.googleapis.com/auth/youtube";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub expires_at: i64,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    expires_in: Option<i64>,
    refresh_token: Option<String>,
    token_type: Option<String>,
    scope: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Extrai client_id e client_secret do JSON baixado do Google Cloud.
pub fn load_client(path: &str) -> Result<(String, String)> {
    let raw = fs::read_to_string(path).with_context(|| {
        format!(
            "nao encontrei {}.\nBaixe o client_secret.json no Google Cloud e coloque nesse caminho.",
            path
        )
    })?;
    let json: Value = serde_json::from_str(&raw).context("client_secret.json invalido")?;
    let node = json
        .get("installed")
        .or_else(|| json.get("web"))
        .ok_or_else(|| anyhow!("client_secret.json sem chave 'installed' ou 'web'"))?;
    let client_id = node
        .get("client_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("client_id ausente"))?
        .to_string();
    let client_secret = node
        .get("client_secret")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("client_secret ausente"))?
        .to_string();
    Ok((client_id, client_secret))
}

/// Salva um client_secret.json no formato esperado pelo Google (Desktop app).
pub fn write_client_secret(path: &str, client_id: &str, client_secret: &str) -> Result<()> {
    let obj = serde_json::json!({
        "installed": {
            "client_id": client_id,
            "client_secret": client_secret,
            "auth_uri": "https://accounts.google.com/o/oauth2/v2/auth",
            "token_uri": "https://oauth2.googleapis.com/token",
            "redirect_uris": ["http://localhost"]
        }
    });
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).ok();
        }
    }
    fs::write(path, serde_json::to_string_pretty(&obj)?)
        .with_context(|| format!("nao consegui salvar {}", path))?;
    Ok(())
}

/// Aceita o JSON completo do Google, valida e salva.
pub fn write_client_secret_raw(path: &str, raw: &str) -> Result<()> {
    let v: Value = serde_json::from_str(raw).context("JSON invalido")?;
    let node = v
        .get("installed")
        .or_else(|| v.get("web"))
        .ok_or_else(|| anyhow!("JSON sem chave 'installed' ou 'web'"))?;
    let cid = node
        .get("client_id")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow!("JSON sem client_id"))?;
    let secret = node
        .get("client_secret")
        .and_then(|x| x.as_str())
        .ok_or_else(|| anyhow!("JSON sem client_secret"))?;
    write_client_secret(path, cid, secret)
}

/// Retorna o client_id atual, se existir.
pub fn peek_client_id(path: &str) -> Option<String> {
    load_client(path).ok().map(|(id, _)| id)
}

pub fn load_token(path: &str) -> Option<Token> {
    let raw = fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn save_token(path: &str, token: &Token) -> Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).ok();
        }
    }
    fs::write(path, serde_json::to_string_pretty(token)?)
        .with_context(|| format!("nao consegui salvar {}", path))?;
    Ok(())
}

pub fn open_browser(url: &str) {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "cmd"
    } else {
        "xdg-open"
    };
    let mut c = std::process::Command::new(cmd);
    if cfg!(target_os = "windows") {
        c.args(["/C", "start", "", url]);
    } else {
        c.arg(url);
    }
    let _ = c.spawn();
}

fn exchange_code(client_id: &str, client_secret: &str, code: &str, redirect: &str) -> Result<Token> {
    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(TOKEN_URI)
        .form(&[
            ("client_id", client_id),
            ("client_secret", client_secret),
            ("code", code),
            ("grant_type", "authorization_code"),
            ("redirect_uri", redirect),
        ])
        .send()
        .context("falha ao chamar o endpoint de token do Google")?;
    let status = resp.status();
    let body: TokenResponse = resp.json().context("resposta de token invalida")?;
    if let Some(err) = body.error {
        bail!(
            "Google recusou o login ({}): {}",
            err,
            body.error_description.unwrap_or_default()
        );
    }
    if !status.is_success() {
        bail!("Google retornou status {}", status);
    }
    let access = body.access_token.ok_or_else(|| anyhow!("sem access_token"))?;
    Ok(Token {
        access_token: access,
        refresh_token: body.refresh_token,
        expires_at: now_unix() + body.expires_in.unwrap_or(3600),
        token_type: body.token_type,
        scope: body.scope,
    })
}

/// Executa o fluxo OAuth completo: abre o navegador e captura o code localmente.
pub fn login(cfg: &Config) -> Result<Token> {
    let (client_id, client_secret) = load_client(&cfg.client_secret)?;

    let listener = TcpListener::bind("127.0.0.1:0").context("nao consegui abrir porta local")?;
    let port = listener.local_addr()?.port();
    let redirect = format!("http://127.0.0.1:{}", port);

    let auth_url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&prompt=consent",
        AUTH_URI,
        urlencoding::encode(&client_id),
        urlencoding::encode(&redirect),
        urlencoding::encode(SCOPE),
    );

    println!("\nAbrindo o navegador para voce autorizar o acesso ao YouTube...");
    println!("Se nao abrir, cole este link no navegador:\n{}\n", auth_url);
    open_browser(&auth_url);

    let code = wait_for_code(&listener)?;

    let token = exchange_code(&client_id, &client_secret, &code, &redirect)?;
    save_token(&cfg.token, &token)?;
    println!("Login concluido e token salvo em {}.", cfg.token);
    Ok(token)
}

fn wait_for_code(listener: &TcpListener) -> Result<String> {
    // Nunca fica preso para sempre: no maximo ~3 minutos.
    listener.set_nonblocking(true).ok();
    let deadline = Instant::now() + Duration::from_secs(180);
    loop {
        if Instant::now() > deadline {
            bail!("tempo esgotado esperando o callback do Google (tente de novo)");
        }
        let (mut stream, _) = match listener.accept() {
            Ok(pair) => pair,
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(200));
                continue;
            }
            Err(e) => return Err(e).context("falha ao receber callback"),
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let mut buf = [0u8; 8192];
        let n = stream.read(&mut buf).unwrap_or(0);
        let req = String::from_utf8_lossy(&buf[..n]);
        let first = req.lines().next().unwrap_or("");
        // GET /?code=... HTTP/1.1
        let path = first.split_whitespace().nth(1).unwrap_or("");
        let query = path.split_once('?').map(|(_, q)| q).unwrap_or("");

        if query.is_empty() || path.starts_with("/favicon") {
            let _ = respond(&mut stream, 404, "Aguardando autorizacao...");
            continue;
        }

        let mut code = None;
        let mut error = None;
        for pair in query.split('&') {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            let val = urlencoding::decode(v).unwrap_or_default().to_string();
            match k {
                "code" => code = Some(val),
                "error" => error = Some(val),
                _ => {}
            }
        }

        if let Some(err) = error {
            let _ = respond(&mut stream, 200, "Autorizacao negada. Pode fechar esta aba.");
            bail!("autorizacao negada pelo usuario: {}", err);
        }
        if let Some(c) = code {
            let _ = respond(
                &mut stream,
                200,
                "Autorizacao concluida! Pode fechar esta aba e voltar ao terminal.",
            );
            return Ok(c);
        }
        let _ = respond(&mut stream, 404, "Requisicao inesperada.");
    }
}

fn respond(stream: &mut std::net::TcpStream, status: u16, msg: &str) -> std::io::Result<()> {
    let body = format!(
        "<html><head><meta charset=\"utf-8\"><title>MaxTube</title></head>\
         <body style=\"font-family:sans-serif;background:#0f0f0f;color:#fff;display:flex;\
         align-items:center;justify-content:center;height:100vh;margin:0\">\
         <h2>{}</h2></body></html>",
        msg
    );
    let resp = format!(
        "HTTP/1.1 {} OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status,
        body.len(),
        body
    );
    stream.write_all(resp.as_bytes())?;
    stream.flush()
}

fn refresh(cfg: &Config, token: &Token) -> Result<Token> {
    let (client_id, client_secret) = load_client(&cfg.client_secret)?;
    let refresh_token = token
        .refresh_token
        .clone()
        .ok_or_else(|| anyhow!("sem refresh_token salvo; rode `maxtube auth` de novo"))?;

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(TOKEN_URI)
        .form(&[
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("refresh_token", refresh_token.as_str()),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .context("falha ao renovar token")?;
    let body: TokenResponse = resp.json().context("resposta de refresh invalida")?;
    if let Some(err) = body.error {
        bail!(
            "falha ao renovar token ({}): {}",
            err,
            body.error_description.unwrap_or_default()
        );
    }
    let access = body.access_token.ok_or_else(|| anyhow!("sem access_token no refresh"))?;
    let refreshed = Token {
        access_token: access,
        refresh_token: Some(refresh_token),
        expires_at: now_unix() + body.expires_in.unwrap_or(3600),
        token_type: body.token_type.or_else(|| token.token_type.clone()),
        scope: body.scope.or_else(|| token.scope.clone()),
    };
    save_token(&cfg.token, &refreshed)?;
    Ok(refreshed)
}

/// Retorna um access_token valido, renovando automaticamente se necessario.
pub fn access_token(cfg: &Config) -> Result<String> {
    let token = load_token(&cfg.token)
        .ok_or_else(|| anyhow!("voce nao esta logado. Rode: maxtube auth"))?;
    if token.expires_at - 60 > now_unix() {
        return Ok(token.access_token);
    }
    println!("Token expirado, renovando...");
    let refreshed = refresh(cfg, &token)?;
    Ok(refreshed.access_token)
}
