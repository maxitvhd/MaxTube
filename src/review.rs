use anyhow::Result;
use std::io::{self, Write};

use crate::config::Config;
use crate::store::{self, Proposal, Status};

pub fn read_line(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut s = String::new();
    io::stdin().read_line(&mut s)?;
    Ok(s.trim().to_string())
}

fn counts(props: &[Proposal]) -> (usize, usize, usize) {
    let mut p = 0;
    let mut a = 0;
    let mut r = 0;
    for x in props {
        match x.status {
            Status::Pending => p += 1,
            Status::Approved => a += 1,
            Status::Rejected => r += 1,
            Status::Applied => {}
        }
    }
    (p, a, r)
}

/// Revisao interativa: aprovar / editar / rejeitar / pular / aprovar-todos / sair.
pub fn run(cfg: &Config) -> Result<()> {
    let mut props = store::load_proposals(cfg)?;
    if props.is_empty() {
        println!("Nenhuma proposta encontrada. Rode `maxtube generate` primeiro.");
        return Ok(());
    }

    let total = props.len();
    let mut approve_rest = false;

    for idx in 0..total {
        if props[idx].status != Status::Pending && !approve_rest {
            continue;
        }
        if approve_rest {
            props[idx].status = Status::Approved;
            store::save_proposals(cfg, &props)?;
            continue;
        }

        loop {
            println!("\n──────────────────────────────────────────────");
            println!("[{} / {}]  video {}", idx + 1, total, props[idx].video_id);
            println!("  Antes:  {}", props[idx].original_title);
            println!("  Depois: {}", props[idx].proposed_title);

            let cmd = read_line("[a]provar  [e]ditar  [r]ejeitar  [s]pular  [A]provar todos restantes  [q]sair > ")?;
            match cmd.as_str() {
                "a" | "" => {
                    props[idx].status = Status::Approved;
                    store::save_proposals(cfg, &props)?;
                    break;
                }
                "e" => {
                    let novo = read_line("Novo titulo: ")?;
                    if !novo.is_empty() {
                        props[idx].proposed_title = novo.chars().take(100).collect();
                    }
                    // continua o loop para decidir de novo
                }
                "r" => {
                    props[idx].status = Status::Rejected;
                    store::save_proposals(cfg, &props)?;
                    break;
                }
                "s" => {
                    break;
                }
                "A" => {
                    approve_rest = true;
                    props[idx].status = Status::Approved;
                    store::save_proposals(cfg, &props)?;
                    break;
                }
                "q" => {
                    println!("Saindo da revisao. Progresso salvo.");
                    let (p, a, r) = counts(&props);
                    println!("Pendentes: {}  Aprovados: {}  Rejeitados: {}", p, a, r);
                    return Ok(());
                }
                _ => {
                    println!("Comando invalido.");
                }
            }
        }
    }

    store::save_proposals(cfg, &props)?;
    let (p, a, r) = counts(&props);
    println!("\nRevisao concluida. Pendentes: {}  Aprovados: {}  Rejeitados: {}", p, a, r);
    Ok(())
}
