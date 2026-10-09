mod auth;
mod cli;
mod config;
mod ollama;
mod review;
mod store;
mod youtube;

use anyhow::{bail, Result};
use clap::Parser;
use std::collections::HashSet;
use std::io::Write;

use cli::{Cli, Command};
use config::Config;
use store::{Proposal, Status};

fn main() {
    if let Err(e) = run() {
        eprintln!("\nErro: {:#}", e);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let cfg = Config::load()?;
    cfg.ensure_dirs()?;

    match cli.command {
        Command::Auth => {
            auth::login(&cfg)?;
        }
        Command::Pull { limit } => do_pull(&cfg, limit)?,
        Command::Generate { model, redo } => do_generate(&cfg, model, redo)?,
        Command::Review => review::run(&cfg)?,
        Command::Apply { yes } => do_apply(&cfg, yes)?,
        Command::Run { limit, model, redo } => {
            do_pull(&cfg, limit)?;
            do_generate(&cfg, model, redo)?;
            review::run(&cfg)?;
            do_apply(&cfg, false)?;
        }
        Command::Status => do_status(&cfg)?,
    }
    Ok(())
}

fn do_pull(cfg: &Config, limit: Option<usize>) -> Result<()> {
    let token = auth::access_token(cfg)?;
    println!("Buscando canal...");
    let playlist = youtube::uploads_playlist(&token)?;
    let ids = youtube::list_video_ids(&token, &playlist, limit)?;
    println!("{} videos encontrados. Buscando detalhes...", ids.len());
    let videos = youtube::get_videos(&token, &ids)?;
    store::save_videos(cfg, &videos)?;
    println!(
        "OK: {} videos salvos em {}",
        videos.len(),
        cfg.data_path("videos.json").display()
    );
    Ok(())
}

fn do_generate(cfg: &Config, model_override: Option<String>, redo: bool) -> Result<()> {
    let model = model_override.unwrap_or_else(|| cfg.model.clone());
    let videos = store::load_videos(cfg)?;
    if videos.is_empty() {
        bail!("nenhum video salvo. Rode `maxtube pull` primeiro.");
    }
    ollama::ensure_ready(&cfg.ollama_url, &model)?;
    let template = cfg.read_prompt()?;

    let mut proposals = if redo {
        Vec::new()
    } else {
        store::load_proposals(cfg)?
    };
    let existing: HashSet<String> = proposals.iter().map(|p| p.video_id.clone()).collect();

    let mut created = 0usize;
    for v in &videos {
        if existing.contains(&v.id) {
            continue;
        }
        let prompt = template
            .replace("{{title}}", &v.title)
            .replace("{{description}}", &v.description);
        print!("Gerando para \"{}\"... ", v.title);
        io_flush();
        match ollama::generate(&cfg.ollama_url, &model, &prompt) {
            Ok(t) => {
                println!("ok");
                proposals.push(Proposal {
                    video_id: v.id.clone(),
                    original_title: v.title.clone(),
                    proposed_title: t,
                    status: Status::Pending,
                });
                created += 1;
                store::save_proposals(cfg, &proposals)?;
            }
            Err(e) => println!("falhou ({})", e),
        }
    }

    println!(
        "\n{} novas sugestoes geradas com o modelo '{}'.",
        created, model
    );
    if created > 0 {
        println!("Revise com: maxtube review");
    }
    Ok(())
}

fn do_apply(cfg: &Config, yes: bool) -> Result<()> {
    let mut proposals = store::load_proposals(cfg)?;
    let mut videos = store::load_videos(cfg)?;

    let targets: Vec<usize> = proposals
        .iter()
        .enumerate()
        .filter(|(_, p)| p.status == Status::Approved)
        .map(|(i, _)| i)
        .collect();

    if targets.is_empty() {
        println!("Nenhuma proposta aprovada. Rode `maxtube review` primeiro.");
        return Ok(());
    }

    if !yes {
        let ans = review::read_line(&format!(
            "Aplicar {} titulo(s) no YouTube? [s/N] ",
            targets.len()
        ))?;
        if !(ans.eq_ignore_ascii_case("s") || ans.eq_ignore_ascii_case("y")) {
            println!("Cancelado.");
            return Ok(());
        }
    }

    let token = auth::access_token(cfg)?;
    let (mut ok, mut fail) = (0usize, 0usize);

    for i in targets {
        let p = proposals[i].clone();
        let video_pos = videos.iter().position(|v| v.id == p.video_id);
        match video_pos {
            Some(pos) => {
                print!("Aplicando: \"{}\" -> \"{}\" ... ", p.original_title, p.proposed_title);
                io_flush();
                match youtube::update_title(&token, &videos[pos], &p.proposed_title) {
                    Ok(_) => {
                        println!("ok");
                        proposals[i].status = Status::Applied;
                        videos[pos].title = p.proposed_title.clone();
                        ok += 1;
                    }
                    Err(e) => {
                        println!("falhou ({})", e);
                        fail += 1;
                    }
                }
            }
            None => {
                println!("video {} nao esta em videos.json; pulando", p.video_id);
                fail += 1;
            }
        }
        store::save_proposals(cfg, &proposals)?;
        store::save_videos(cfg, &videos)?;
    }

    println!("\nAplicados: {}  Falhas: {}", ok, fail);
    Ok(())
}

fn do_status(cfg: &Config) -> Result<()> {
    let videos = store::load_videos(cfg)?;
    let proposals = store::load_proposals(cfg)?;
    let (mut p, mut a, mut r, mut ap) = (0, 0, 0, 0);
    for x in &proposals {
        match x.status {
            Status::Pending => p += 1,
            Status::Approved => a += 1,
            Status::Rejected => r += 1,
            Status::Applied => ap += 1,
        }
    }
    println!("Modelo Ollama : {}", cfg.model);
    println!("Prompt        : {}", cfg.prompt_file);
    println!("Videos salvos : {}", videos.len());
    println!(
        "Propostas     : {} (pendentes {}, aprovadas {}, rejeitadas {}, aplicadas {})",
        proposals.len(),
        p,
        a,
        r,
        ap
    );
    println!(
        "Login         : {}",
        if auth::load_token(&cfg.token).is_some() {
            "sim"
        } else {
            "nao (rode `maxtube auth`)"
        }
    );
    Ok(())
}

fn io_flush() {
    let _ = std::io::stdout().flush();
}
