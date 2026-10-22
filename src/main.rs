//! ArchForge : éditeur de la base de données d'ArchCore.
//!
//! Un seul exécutable : il sert l'interface (embarquée) sur 127.0.0.1 et ouvre le navigateur. L'interface
//! parle à MariaDB par l'API de `api.rs` ; toute requête qu'elle produit est affichée avant d'être exécutée
//! ou ajoutée à la couche de contenu d'ArchCore.

mod api;
mod archcore;
mod content;
mod db;

use anyhow::{bail, Result};
use std::net::SocketAddr;
use std::path::PathBuf;

const USAGE: &str = "\
ArchForge — éditeur de la base de données d'ArchCore

Utilisation : archforge [options]

  --archcore <dossier>   dépôt ArchCore (config/world.conf, content/) ; par défaut, le dossier courant
                         s'il contient config/world.conf, sinon la variable ARCHCORE_DIR
  --port <n>             port local de l'interface (8787 par défaut)
  --no-browser           ne pas ouvrir le navigateur
  -h, --help             cette aide
";

struct Args {
    archcore: Option<PathBuf>,
    port: u16,
    browser: bool,
}

fn parse_args() -> Result<Args> {
    let mut args = Args { archcore: None, port: 8787, browser: true };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--archcore" => args.archcore = it.next().map(PathBuf::from),
            "--port" => {
                let Some(p) = it.next().and_then(|p| p.parse().ok()) else { bail!("--port attend un nombre") };
                args.port = p;
            }
            "--no-browser" => args.browser = false,
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            other => bail!("option inconnue : {other}\n\n{USAGE}"),
        }
    }
    Ok(args)
}

fn open_browser(url: &str) {
    let result = if cfg!(target_os = "windows") {
        std::process::Command::new("cmd").args(["/C", "start", "", url]).spawn()
    } else if cfg!(target_os = "macos") {
        std::process::Command::new("open").arg(url).spawn()
    } else {
        std::process::Command::new("xdg-open").arg(url).spawn()
    };
    if result.is_err() {
        println!("Ouvrez {url} dans votre navigateur.");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args()?;
    let archcore = archcore::locate(args.archcore.clone());
    match &archcore {
        Some(dir) => println!("Dépôt ArchCore : {}", dir.display()),
        None => println!("Aucun dépôt ArchCore trouvé (--archcore) : connexion à saisir, couche de contenu désactivée."),
    }

    let state = api::AppState::new(archcore);
    let addr = SocketAddr::from(([127, 0, 0, 1], args.port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let url = format!("http://{addr}/");
    println!("ArchForge écoute sur {url} (Ctrl+C pour arrêter)");
    if args.browser {
        open_browser(&url);
    }
    axum::serve(listener, api::router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
