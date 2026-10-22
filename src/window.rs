//! Fenêtre de l'application : l'interface s'affiche dans une vue web intégrée (WebView2 sous Windows,
//! WebKit sous macOS et Linux), comme Keira3 dans Electron, sans navigateur ni barre d'adresse.
//!
//! La vue charge l'interface servie par `api.rs` sur 127.0.0.1. Les liens vers d'autres sites s'ouvrent
//! dans le navigateur du système ; la fermeture passe par la page, qui demande confirmation s'il reste
//! des modifications non enregistrées.

use anyhow::{Context, Result};
use std::path::PathBuf;
use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::window::{Icon, WindowBuilder};
use wry::{NewWindowResponse, WebContext, WebViewBuilder};

enum UserEvent {
    Close,
}

/// Icône de la fenêtre, tirée de l'emblème embarqué.
fn icon() -> Option<Icon> {
    let file = crate::api::asset_bytes("img/emblem-64.png")?;
    let mut decoder = png::Decoder::new(std::io::Cursor::new(file));
    decoder.set_transformations(png::Transformations::EXPAND);
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let pixels = &buf[..info.buffer_size()];
    let rgba = match info.color_type {
        png::ColorType::Rgba => pixels.to_vec(),
        png::ColorType::Rgb => pixels.chunks(3).flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        _ => return None,
    };
    Icon::from_rgba(rgba, info.width, info.height).ok()
}

/// Dossier des données de la vue web (connexions récentes, filtres…), propre à l'utilisateur.
pub fn data_dir() -> PathBuf {
    let base = if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library").join("Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local").join("share")))
    };
    base.unwrap_or_else(std::env::temp_dir).join(if cfg!(target_os = "linux") { "archforge" } else { "ArchForge" })
}

/// Ouvre la fenêtre sur `url` et ne rend la main qu'à sa fermeture (fin du programme).
pub fn run(url: &str) -> Result<()> {
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title("ArchForge")
        .with_inner_size(LogicalSize::new(1400.0, 900.0))
        .with_min_inner_size(LogicalSize::new(900.0, 600.0))
        .with_window_icon(icon())
        .build(&event_loop)
        .context("création de la fenêtre")?;

    let mut context = WebContext::new(Some(data_dir()));
    let origin = url.trim_end_matches('/').to_string();
    let builder = WebViewBuilder::new_with_web_context(&mut context)
        .with_url(url)
        .with_devtools(cfg!(debug_assertions))
        .with_navigation_handler(move |target| {
            let local = target.starts_with(&origin) || target.starts_with("about:");
            if !local {
                crate::open_browser(&target);
            }
            local
        })
        .with_new_window_req_handler(|target, _| {
            crate::open_browser(&target);
            NewWindowResponse::Deny
        })
        .with_ipc_handler(move |request| {
            if request.body() == "close" {
                let _ = proxy.send_event(UserEvent::Close);
            }
        });

    #[cfg(any(target_os = "windows", target_os = "macos"))]
    let webview = builder.build(&window);
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        builder.build_gtk(window.default_vbox().expect("boîte GTK de la fenêtre"))
    };
    let webview = webview.context(if cfg!(target_os = "windows") {
        "affichage impossible : le composant Microsoft Edge WebView2 manque (https://go.microsoft.com/fwlink/p/?LinkId=2124703), ou lancer ArchForge avec --browser"
    } else {
        "affichage impossible : installer WebKitGTK 4.1 (paquet webkit2gtk-4.1), ou lancer ArchForge avec --browser"
    })?;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            // La page décide : elle confirme s'il reste des modifications, puis renvoie « close ».
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                let asked = webview.evaluate_script(
                    "window.archforgeRequestClose ? window.archforgeRequestClose() : window.ipc.postMessage('close')",
                );
                if asked.is_err() {
                    *control_flow = ControlFlow::Exit;
                }
            }
            Event::UserEvent(UserEvent::Close) => *control_flow = ControlFlow::Exit,
            _ => {}
        }
    });
}
