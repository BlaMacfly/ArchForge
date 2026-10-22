//! ArchForge : éditeur de la base de données d'ArchCore.
//!
//! Un seul exécutable : il sert l'interface (embarquée) sur 127.0.0.1 et l'affiche dans sa propre fenêtre
//! (`window.rs`). L'interface parle à MariaDB par l'API de `api.rs` ; toute requête qu'elle produit est
//! affichée avant d'être exécutée ou ajoutée à la couche de contenu d'ArchCore.

// Application fenêtrée : pas de console sous Windows (gardée en débogage pour les journaux).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod api;
mod archcore;
mod content;
mod db;
mod window;

use anyhow::{bail, Result};
use std::net::SocketAddr;
use std::path::PathBuf;

const USAGE: &str = "\
ArchForge — éditeur de la base de données d'ArchCore

Utilisation : archforge [options]

  --archcore <dossier>   dépôt ArchCore (config/world.conf, content/) ; par défaut, le dossier courant
                         s'il contient config/world.conf, sinon la variable ARCHCORE_DIR
  --port <n>             port local de l'interface (8787 par défaut, sinon le premier libre après)
  --browser              afficher l'interface dans le navigateur plutôt que dans la fenêtre d'ArchForge
  --serve                servir l'interface sans rien ouvrir (accès par http://127.0.0.1:<port>/)
  -h, --help             cette aide
";

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Window,
    Browser,
    Serve,
}

struct Args {
    archcore: Option<PathBuf>,
    port: u16,
    port_explicit: bool,
    mode: Mode,
}

fn parse_args() -> Result<Args> {
    let mut args = Args { archcore: None, port: 8787, port_explicit: false, mode: Mode::Window };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--archcore" => args.archcore = it.next().map(PathBuf::from),
            "--port" => {
                let Some(p) = it.next().and_then(|p| p.parse().ok()) else { bail!("--port attend un nombre") };
                args.port = p;
                args.port_explicit = true;
            }
            "--browser" => args.mode = Mode::Browser,
            // --no-browser : nom de la version 0.1.
            "--serve" | "--no-browser" => args.mode = Mode::Serve,
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            other => bail!("option inconnue : {other}\n\n{USAGE}"),
        }
    }
    Ok(args)
}

/// Ouvre une adresse dans le navigateur du système (liens externes, mode --browser).
pub fn open_browser(url: &str) {
    let mut command = if cfg!(target_os = "windows") {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", "", url]);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(url);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(url);
        c
    };
    #[cfg(windows)]
    {
        // Sans fenêtre de console éphémère pour « cmd ».
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x0800_0000);
    }
    if command.spawn().is_err() {
        println!("Ouvrez {url} dans votre navigateur.");
    }
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

fn run(args: Args) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build()?;
    let archcore = archcore::locate(args.archcore.clone());
    match &archcore {
        Some(dir) => println!("Dépôt ArchCore : {}", dir.display()),
        None => println!("Aucun dépôt ArchCore trouvé (--archcore) : connexion à saisir, couche de contenu désactivée."),
    }

    // Dans le navigateur, un lancement de trop rouvre simplement la page de l'instance qui tourne.
    let wanted = SocketAddr::from(([127, 0, 0, 1], args.port));
    if args.mode == Mode::Browser && runtime.block_on(archforge_at(wanted)) {
        let url = format!("http://{wanted}/");
        println!("ArchForge tourne déjà sur {url} : ouverture de la page.");
        open_browser(&url);
        return Ok(());
    }

    let listener = runtime.block_on(bind(args.port, args.port_explicit))?;
    let url = format!("http://{}/", listener.local_addr()?);
    let router = api::router(api::AppState::new(archcore));
    println!("ArchForge écoute sur {url}");

    if args.mode == Mode::Window {
        // Le serveur tourne en tâche de fond ; la fenêtre occupe le fil principal jusqu'à sa fermeture.
        runtime.spawn(async move {
            let _ = axum::serve(listener, router).await;
        });
        return window::run(&url);
    }

    if args.mode == Mode::Browser {
        open_browser(&url);
    }
    println!("Ctrl+C pour arrêter.");
    runtime.block_on(async {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
    })?;
    Ok(())
}

/// Sous Windows, une application fenêtrée n'a pas de console : lancée depuis un terminal, elle s'attache à
/// celui-ci pour que --help et les messages s'y affichent.
#[cfg(windows)]
fn attach_parent_console() {
    extern "system" {
        fn AttachConsole(process: u32) -> i32;
    }
    // SAFETY : appel sans pointeur ; ATTACH_PARENT_PROCESS = (DWORD)-1.
    unsafe {
        AttachConsole(u32::MAX);
    }
}

fn main() {
    #[cfg(windows)]
    attach_parent_console();
    let args = match parse_args() {
        Ok(args) => args,
        Err(e) => fatal(&e, Mode::Window),
    };
    let mode = args.mode;
    if let Err(e) = run(args) {
        fatal(&e, mode);
    }
}

/// Message d'erreur dans le terminal, et dans une boîte de dialogue quand ArchForge a été lancé sans
/// terminal (double-clic).
fn fatal(error: &anyhow::Error, mode: Mode) -> ! {
    let message = format!("{error:#}");
    eprintln!("Erreur : {message}");
    if mode == Mode::Window {
        rfd::MessageDialog::new()
            .set_level(rfd::MessageLevel::Error)
            .set_title("ArchForge")
            .set_description(&message)
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }
    std::process::exit(1);
}
