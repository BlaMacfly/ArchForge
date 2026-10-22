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
  --port <n>             port local de l'interface (8787 par défaut, sinon le premier libre après)
  --no-browser           ne pas ouvrir le navigateur
  -h, --help             cette aide
";

struct Args {
    archcore: Option<PathBuf>,
    port: u16,
    port_explicit: bool,
    browser: bool,
}

fn parse_args() -> Result<Args> {
    let mut args = Args { archcore: None, port: 8787, port_explicit: false, browser: true };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--archcore" => args.archcore = it.next().map(PathBuf::from),
            "--port" => {
                let Some(p) = it.next().and_then(|p| p.parse().ok()) else { bail!("--port attend un nombre") };
                args.port = p;
                args.port_explicit = true;
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

/// Vrai si la console n'appartient qu'à ArchForge : lancé d'un double-clic, la fenêtre se fermerait avec
/// le programme, avant qu'on ait pu lire un message d'erreur.
#[cfg(windows)]
fn own_console() -> bool {
    extern "system" {
        fn GetConsoleProcessList(list: *mut u32, count: u32) -> u32;
    }
    let mut list = [0u32; 2];
    // SAFETY : la liste a la taille annoncée ; la fonction n'écrit pas au-delà.
    unsafe { GetConsoleProcessList(list.as_mut_ptr(), list.len() as u32) == 1 }
}

#[cfg(not(windows))]
fn own_console() -> bool {
    false
}

/// Une instance d'ArchForge répond-elle déjà sur ce port ?
async fn archforge_at(addr: SocketAddr) -> bool {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let probe = async {
        let mut stream = tokio::net::TcpStream::connect(addr).await.ok()?;
        stream.write_all(format!("GET / HTTP/1.0\r\nHost: {addr}\r\n\r\n").as_bytes()).await.ok()?;
        let mut page = Vec::new();
        stream.read_to_end(&mut page).await.ok()?;
        Some(String::from_utf8_lossy(&page).contains("archforge-token"))
    };
    matches!(tokio::time::timeout(std::time::Duration::from_secs(2), probe).await, Ok(Some(true)))
}

/// Le port demandé, sinon (port par défaut seulement) l'un des vingt suivants.
async fn bind(port: u16, explicit: bool) -> Result<tokio::net::TcpListener> {
    let last = if explicit { port } else { port.saturating_add(20) };
    let mut error = None;
    for p in port..=last {
        match tokio::net::TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], p))).await {
            Ok(listener) => return Ok(listener),
            Err(e) => error = Some(e),
        }
    }
    let e = error.expect("au moins un port essayé");
    if explicit {
        bail!("le port {port} est déjà utilisé ({e}) : en choisir un autre avec --port");
    }
    bail!("les ports {port} à {last} sont déjà utilisés ({e}) : en choisir un autre avec --port");
}

async fn run(args: Args) -> Result<()> {
    let archcore = archcore::locate(args.archcore.clone());
    match &archcore {
        Some(dir) => println!("Dépôt ArchCore : {}", dir.display()),
        None => println!("Aucun dépôt ArchCore trouvé (--archcore) : connexion à saisir, couche de contenu désactivée."),
    }

    // Un double-clic de trop : ArchForge tourne déjà, on rouvre simplement sa page.
    let wanted = SocketAddr::from(([127, 0, 0, 1], args.port));
    if archforge_at(wanted).await {
        let url = format!("http://{wanted}/");
        println!("ArchForge tourne déjà sur {url} : ouverture de la page.");
        if args.browser {
            open_browser(&url);
        }
        return Ok(());
    }

    let state = api::AppState::new(archcore);
    let listener = bind(args.port, args.port_explicit).await?;
    let url = format!("http://{}/", listener.local_addr()?);
    println!("ArchForge écoute sur {url}");
    println!("Laisser cette fenêtre ouverte pendant l'utilisation ; Ctrl+C ou fermer la fenêtre pour arrêter.");
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

#[tokio::main]
async fn main() {
    let result = match parse_args() {
        Ok(args) => run(args).await,
        Err(e) => Err(e),
    };
    if let Err(e) = result {
        eprintln!("Erreur : {e:#}");
        if own_console() {
            eprintln!("\nAppuyer sur Entrée pour fermer.");
            let _ = std::io::stdin().read_line(&mut String::new());
        }
        std::process::exit(1);
    }
}
