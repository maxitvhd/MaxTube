use anyhow::{Context, Result};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};

use crate::auth;
use crate::config::Config;

const STYLE: &str = r#"
:root{color-scheme:dark}
*{box-sizing:border-box}
body{margin:0;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,sans-serif;
background:#0f0f0f;color:#f1f1f1;display:flex;min-height:100vh;align-items:center;justify-content:center;padding:24px}
.card{background:#1b1b1b;border:1px solid #2f2f2f;border-radius:16px;max-width:640px;width:100%;
padding:32px;box-shadow:0 12px 40px rgba(0,0,0,.5)}
h1{margin:0 0 4px;font-size:24px}
h1 .r{color:#ff0033}
p.sub{margin:0 0 20px;color:#aaa;font-size:14px}
.row{display:flex;gap:10px;align-items:center;margin:10px 0;padding:10px 12px;border-radius:10px;background:#141414;border:1px solid #262626;font-size:14px}
.dot{width:10px;height:10px;border-radius:50%;flex:0 0 auto}
.ok{background:#2ecc71}.no{background:#e74c3c}
label{display:block;font-size:13px;color:#bbb;margin:16px 0 6px}
input,textarea{width:100%;padding:11px 12px;border-radius:10px;border:1px solid #333;background:#111;color:#fff;font-size:14px}
textarea{min-height:120px;font-family:ui-monospace,Menlo,monospace;resize:vertical}
fieldset{border:1px solid #2a2a2a;border-radius:12px;padding:8px 16px 16px;margin:18px 0}
legend{color:#888;font-size:12px;padding:0 8px;text-transform:uppercase;letter-spacing:.08em}
button,.btn{cursor:pointer;border:0;border-radius:10px;padding:12px 18px;font-size:15px;font-weight:600;
background:#ff0033;color:#fff;text-decoration:none;display:inline-block}
button:hover,.btn:hover{background:#e0002d}
.btn.ghost{background:#2a2a2a}.btn.ghost:hover{background:#3a3a3a}
.actions{margin-top:22px;display:flex;gap:10px;flex-wrap:wrap}
.err{background:#3a1414;border:1px solid #7a1f1f;color:#ffb4b4;padding:12px;border-radius:10px;margin:12px 0;font-size:14px}
.banner{background:#12331f;border:1px solid #1f6b3d;color:#b7f0cd;padding:12px;border-radius:10px;margin:12px 0;font-size:14px}
code{background:#000;padding:2px 6px;border-radius:6px;font-size:12px;color:#ff9db0}
small{color:#888}
"#;

fn page(inner: &str) -> String {
    format!(
        "<!doctype html><html lang=\"pt-br\"><head><meta charset=\"utf-8\">\
        <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
        <title>MaxTube - Painel</title><style>{}</style></head><body>\
        <div class=\"card\"><h1>Max<span class=\"r\">Tube</span> <small>painel</small></h1>{}</div>\
        </body></html>",
        STYLE, inner
    )
}

fn open_browser(url: &str) {
    auth::open_browser(url);
}

pub fn run(cfg: &Config) -> Result<()> {
    cfg.ensure_dirs()?;
    let listener = TcpListener::bind("127.0.0.1:8787")
        .or_else(|_| TcpListener::bind("127.0.0.1:0"))
        .context("nao consegui abrir a porta do painel")?;
    let port = listener.local_addr()?.port();
    let url = format!("http://127.0.0.1:{}", port);
    println!("\nPainel MaxTube disponivel em: {}", url);
    println!("(Ctrl+C para encerrar)\n");
    open_browser(&url);

    for stream in listener.incoming() {
        match stream {
            Ok(mut s) => {
                if let Err(e) = handle(cfg, &mut s) {
                    eprintln!("painel: {}", e);
                }
            }
            Err(_) => continue,
        }
    }
    Ok(())
}

fn handle(cfg: &Config, stream: &mut TcpStream) -> Result<()> {
    let (method, target, body) = match read_request(stream)? {
        Some(r) => r,
        None => return Ok(()),
    };
    let path = target.split('?').next().unwrap_or("/").to_string();
    let query = target.split_once('?').map(|(_, q)| q.to_string()).unwrap_or_default();

    match (method.as_str(), path.as_str()) {
        ("GET", "/") => {
            let ok = query.split('&').any(|p| p == "ok=1");
            respond(stream, 200, "OK", "text/html; charset=utf-8", &home(cfg, ok))?;
        }
        ("POST", "/save") => {
            let form = parse_form(&body);
            let json = get(&form, "json").unwrap_or("").trim().to_string();
            let result = if !json.is_empty() {
                auth::write_client_secret_raw(&cfg.client_secret, &json)
            } else {
                let id = get(&form, "client_id").unwrap_or("").trim();
                let secret = get(&form, "client_secret").unwrap_or("").trim();
                if id.is_empty() || secret.is_empty() {
                    Err(anyhow::anyhow!("preencha o Client ID e o Client Secret (ou cole o JSON completo)"))
                } else {
                    auth::write_client_secret(&cfg.client_secret, id, secret)
                }
            };
            match result {
                Ok(_) => redirect(stream, "/?ok=1")?,
                Err(e) => {
                    let inner = format!(
                        "<div class=\"err\">Erro ao salvar: {}</div>\
                         <div class=\"actions\"><a class=\"btn ghost\" href=\"/\">Voltar</a></div>",
                        html_escape(&format!("{:#}", e))
                    );
                    respond(stream, 200, "OK", "text/html; charset=utf-8", &page(&inner))?;
                }
            }
        }
        ("GET", "/login") => {
            println!("Iniciando login OAuth...");
            let inner = match auth::login(cfg) {
                Ok(_) => format!(
                    "<div class=\"banner\">Login concluido! Token salvo em <code>{}</code>.</div>\
                     <div class=\"actions\"><a class=\"btn\" href=\"/\">Voltar ao painel</a></div>",
                    html_escape(&cfg.token)
                ),
                Err(e) => format!(
                    "<div class=\"err\">Falha no login: {}</div>\
                     <div class=\"actions\"><a class=\"btn ghost\" href=\"/\">Voltar</a></div>",
                    html_escape(&format!("{:#}", e))
                ),
            };
            respond(stream, 200, "OK", "text/html; charset=utf-8", &page(&inner))?;
        }
        ("GET", "/favicon.ico") => respond(stream, 204, "No Content", "text/plain", "")?,
        _ => respond(stream, 404, "Not Found", "text/plain; charset=utf-8", "nao encontrado")?,
    }
    Ok(())
}

fn home(cfg: &Config, saved: bool) -> String {
    let has_creds = auth::peek_client_id(&cfg.client_secret);
    let logged = auth::load_token(&cfg.token).is_some();

    let creds_row = match &has_creds {
        Some(id) => format!(
            "<div class=\"row\"><span class=\"dot ok\"></span>Credenciais salvas: <code>{}</code></div>",
            html_escape(&mask(id))
        ),
        None => "<div class=\"row\"><span class=\"dot no\"></span>Nenhuma credencial salva ainda</div>".to_string(),
    };
    let login_row = format!(
        "<div class=\"row\"><span class=\"dot {}\"></span>Conta do YouTube: {}</div>",
        if logged { "ok" } else { "no" },
        if logged { "autenticada" } else { "nao autenticada" }
    );
    let banner = if saved {
        "<div class=\"banner\">Credenciais salvas com sucesso!</div>"
    } else {
        ""
    };
    let login_btn = if has_creds.is_some() {
        "<a class=\"btn\" href=\"/login\">Fazer login com Google</a>"
    } else {
        ""
    };

    format!(
        "<p class=\"sub\">Cole as credenciais do Google Cloud (OAuth - App para computador) e faca login.</p>\
         {banner}{creds_row}{login_row}\
         <form method=\"POST\" action=\"/save\">\
           <fieldset><legend>Opcao 1 - colar o JSON completo</legend>\
             <textarea name=\"json\" placeholder='Cole aqui o conteudo do client_secret.json'></textarea>\
           </fieldset>\
           <fieldset><legend>Opcao 2 - informar os campos</legend>\
             <label>Client ID</label><input name=\"client_id\" autocomplete=\"off\" placeholder=\"...apps.googleusercontent.com\">\
             <label>Client Secret</label><input name=\"client_secret\" autocomplete=\"off\" placeholder=\"GOCSPX-...\">\
           </fieldset>\
           <div class=\"actions\"><button type=\"submit\">Salvar credenciais</button>{login_btn}</div>\
         </form>\
         <p style=\"margin-top:18px\"><small>As credenciais ficam salvas localmente em <code>{}</code> (fora do git).</small></p>",
        html_escape(&cfg.client_secret),
        banner = banner,
        creds_row = creds_row,
        login_row = login_row,
        login_btn = login_btn,
    )
}

fn mask(s: &str) -> String {
    if s.len() <= 12 {
        return s.to_string();
    }
    format!("{}...{}", &s[..8], &s[s.len() - 12..])
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn parse_form(body: &str) -> Vec<(String, String)> {
    body.split('&')
        .filter_map(|p| {
            let (k, v) = p.split_once('=')?;
            Some((decode(k), decode(v)))
        })
        .collect()
}

fn decode(s: &str) -> String {
    urlencoding::decode(&s.replace('+', " ")).unwrap_or_default().to_string()
}

fn get<'a>(form: &'a [(String, String)], key: &str) -> Option<&'a str> {
    form.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

fn respond(stream: &mut TcpStream, status: u16, reason: &str, ctype: &str, body: &str) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        reason,
        ctype,
        body.as_bytes().len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(body.as_bytes())?;
    stream.flush()
}

fn redirect(stream: &mut TcpStream, location: &str) -> std::io::Result<()> {
    let head = format!(
        "HTTP/1.1 303 See Other\r\nLocation: {}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
        location
    );
    stream.write_all(head.as_bytes())?;
    stream.flush()
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<Option<(String, String, String)>> {
    let mut buf: Vec<u8> = Vec::new();
    let mut tmp = [0u8; 8192];
    let header_end = loop {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            return Ok(None);
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = find(&buf, b"\r\n\r\n") {
            break pos;
        }
        if buf.len() > 2_000_000 {
            return Ok(None);
        }
    };

    let header = String::from_utf8_lossy(&buf[..header_end]).to_string();
    let mut lines = header.lines();
    let first = lines.next().unwrap_or("");
    let mut parts = first.split_whitespace();
    let method = parts.next().unwrap_or("GET").to_string();
    let target = parts.next().unwrap_or("/").to_string();

    let mut content_length = 0usize;
    for l in lines {
        if let Some((k, v)) = l.split_once(':') {
            if k.eq_ignore_ascii_case("content-length") {
                content_length = v.trim().parse().unwrap_or(0);
            }
        }
    }

    let mut body = buf[header_end + 4..].to_vec();
    while body.len() < content_length {
        let n = stream.read(&mut tmp)?;
        if n == 0 {
            break;
        }
        body.extend_from_slice(&tmp[..n]);
    }

    Ok(Some((
        method,
        target,
        String::from_utf8_lossy(&body).to_string(),
    )))
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}
