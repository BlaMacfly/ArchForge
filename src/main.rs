//! ArchForge : éditeur de la base de données d'ArchCore (émulateur de serveur ArchLord), sur le modèle de Keira3.
//!
//! Application de bureau en Rust (egui) : elle se connecte à MariaDB, présente les tables du monde d'ArchCore
//! dans des éditeurs, et produit le SQL des modifications — à exécuter sur la base ou à ajouter à la couche
//! de contenu d'ArchCore (`content/`), qui survit aux réimports.

// Application fenêtrée : pas de console sous Windows (gardée en débogage pour les journaux).
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod archcore;
mod content;
mod db;
mod defs;
mod editor;
mod model;
mod sql;
mod task;
mod ui;

use anyhow::{bail, Result};
use std::path::PathBuf;

const USAGE: &str = "\
ArchForge — éditeur de la base de données d'ArchCore

Utilisation : archforge [--archcore <dossier>] [--connect] [--open <éditeur>[/<id>[/<onglet>]]]

  --archcore <dossier>   dépôt ArchCore (config/world.conf, content/) ; par défaut, le dossier courant
                         s'il contient config/world.conf, sinon la variable ARCHCORE_DIR
  --connect              se connecter tout de suite avec la configuration de world.conf
  --open <éditeur>[/<id>[/<onglet>]]
                         ouvrir un éditeur ou une entrée une fois connecté : quest, item/1, creature/459/loot
  -h, --help             cette aide
";

fn parse_args() -> Result<(Option<PathBuf>, ui::Startup)> {
    let mut archcore = None;
    let mut startup = ui::Startup::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--archcore" => archcore = it.next().map(PathBuf::from),
            "--connect" => startup.connect = true,
            "--open" => startup.open = it.next(),
            "-h" | "--help" => {
                print!("{USAGE}");
                std::process::exit(0);
            }
            other => bail!("option inconnue : {other}\n\n{USAGE}"),
        }
    }
    Ok((archcore, startup))
}

/// Sous Windows, une application fenêtrée n'a pas de console : lancée depuis un terminal, elle s'attache à
/// celui-ci pour que --help et les erreurs s'y affichent.
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

fn fatal(message: &str) -> ! {
    eprintln!("Erreur : {message}");
    rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("ArchForge")
        .set_description(message)
        .set_buttons(rfd::MessageButtons::Ok)
        .show();
    std::process::exit(1);
}

fn main() {
    #[cfg(windows)]
    attach_parent_console();
    let (archcore, startup) = match parse_args() {
        Ok((option, startup)) => (archcore::locate(option), startup),
        Err(e) => fatal(&format!("{e:#}")),
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("ArchForge")
        .with_inner_size([1440.0, 900.0])
        .with_min_inner_size([1000.0, 640.0]);
    if let Some((rgba, width, height)) = ui::decode_png(include_bytes!("../assets/emblem-64.png")) {
        viewport = viewport.with_icon(egui::IconData { rgba, width, height });
    }
    let options = eframe::NativeOptions { viewport, ..Default::default() };
    let result = eframe::run_native("ArchForge", options, Box::new(move |cc| Ok(Box::new(ui::App::new(cc, archcore, startup)))));
    if let Err(e) = result {
        fatal(&format!("Ouverture de la fenêtre impossible : {e}"));
    }
}
