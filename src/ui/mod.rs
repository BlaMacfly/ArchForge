//! Application de bureau (egui) : barre latérale des éditeurs, pages, fenêtres et préférences.

pub mod common;
mod editor_view;
mod pages;
mod search;
mod widgets;

use egui::{RichText, Ui};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use common::{muted, primary, Confirm, ConfirmAction, Modals, RefCache, Toasts};
use editor_view::{EditorRequest, EditorView};
use pages::{ConnectView, ContentDialog, ContentView, DashboardAction, DashboardView, SqlView};
use search::SearchView;
use widgets::{Env, Nav};

use crate::db::{Db, Settings};
use crate::defs::{by_route, ENTITIES};
use crate::editor::display;

/// Préférences gardées d'une session à l'autre.
#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub connections: Vec<Settings>,
    pub remember_password: bool,
    /// Entrées récentes par éditeur (identifiants en texte).
    pub recent: HashMap<String, Vec<String>>,
    pub filters: HashMap<String, Vec<String>>,
    pub sql_text: String,
    pub sql_db: String,
    pub content_dir: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Screen {
    Connect,
    Dashboard,
    Search,
    Editor,
    Sql,
    Content,
}

/// Destination d'une navigation.
enum Target {
    Screen(Screen),
    Search(usize),
    Editor { entity: usize, id: Value, is_new: bool, copy_from: Option<Value> },
    Disconnect,
}

pub struct App {
    archcore: Option<PathBuf>,
    content: Option<PathBuf>,
    prefs: Prefs,
    db: Option<Arc<Db>>,
    screen: Screen,
    connect: ConnectView,
    dashboard: DashboardView,
    search: Option<SearchView>,
    editor: Option<EditorView>,
    sql: SqlView,
    content_view: ContentView,
    refs: RefCache,
    modals: Modals,
    toasts: Toasts,
    content_dialog: Option<ContentDialog>,
    pending: Option<Target>,
    allow_close: bool,
    logo: Option<egui::TextureHandle>,
    emblem: Option<egui::TextureHandle>,
    background: Option<egui::TextureHandle>,
    ctx: egui::Context,
    startup: Startup,
    /// Onglet à ouvrir avec l'entrée demandée au démarrage.
    open_tab: Option<String>,
}

/// Démarrage demandé en ligne de commande : connexion immédiate, entrée à ouvrir (« item/1 », « creature »).
#[derive(Default)]
pub struct Startup {
    pub connect: bool,
    pub open: Option<String>,
}

/// Image PNG embarquée → pixels RGBA.
pub fn decode_png(bytes: &[u8]) -> Option<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(bytes));
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
    Some((rgba, info.width, info.height))
}

fn texture(ctx: &egui::Context, name: &str, bytes: &[u8]) -> Option<egui::TextureHandle> {
    let (rgba, w, h) = decode_png(bytes)?;
    let image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
    Some(ctx.load_texture(name, image, egui::TextureOptions::LINEAR))
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, archcore: Option<PathBuf>, startup: Startup) -> Self {
        common::apply_theme(&cc.egui_ctx);
        let prefs: Prefs = cc.storage.and_then(|s| eframe::get_value(s, eframe::APP_KEY)).unwrap_or_default();
        // Le dépôt ArchCore désigné au lancement l'emporte sur le dossier retenu la fois précédente.
        let content = archcore
            .as_ref()
            .map(|d| d.join("content"))
            .filter(|p| p.is_dir())
            .or_else(|| prefs.content_dir.as_ref().map(PathBuf::from).filter(|p| p.is_dir()));
        let defaults = crate::archcore::default_settings(archcore.as_deref());
        let mut connect = ConnectView::new(defaults.clone(), &prefs);
        if startup.connect {
            connect.settings = defaults;
            connect.start(&cc.egui_ctx);
        }
        App {
            connect,
            archcore,
            content,
            prefs,
            db: None,
            screen: Screen::Connect,
            dashboard: DashboardView::default(),
            search: None,
            editor: None,
            sql: SqlView::new(),
            content_view: ContentView::default(),
            refs: RefCache::default(),
            modals: Modals::default(),
            toasts: Toasts::default(),
            content_dialog: None,
            pending: None,
            allow_close: false,
            logo: texture(&cc.egui_ctx, "logo", include_bytes!("../../assets/logo.png")),
            emblem: texture(&cc.egui_ctx, "emblem", include_bytes!("../../assets/emblem-64.png")),
            background: texture(&cc.egui_ctx, "background", include_bytes!("../../assets/login-background.png")),
            ctx: cc.egui_ctx.clone(),
            startup,
            open_tab: None,
        }
    }

    fn editor_dirty(&self) -> bool {
        self.screen == Screen::Editor && self.editor.as_ref().is_some_and(|e| e.dirty())
    }

    /// Navigation, avec confirmation si l'entrée ouverte a des modifications non enregistrées.
    fn go(&mut self, target: Target) {
        let leaving = !matches!(target, Target::Editor { .. }) || self.editor.is_some();
        if leaving && self.editor_dirty() {
            let disconnect = matches!(target, Target::Disconnect);
            self.pending = Some(target);
            self.modals.confirm = Some(Confirm {
                title: "Modifications non enregistrées".into(),
                message: if disconnect { "Se déconnecter sans enregistrer les modifications de cette entrée ?".into() } else { "Quitter cette entrée sans enregistrer ses modifications ?".into() },
                ok: "Quitter".into(),
                action: ConfirmAction::Leave,
            });
            return;
        }
        self.apply(target);
    }

    fn apply(&mut self, target: Target) {
        let Some(db) = self.db.clone() else {
            self.screen = Screen::Connect;
            return;
        };
        match target {
            Target::Screen(s) => {
                if s == Screen::Dashboard {
                    self.dashboard.refresh();
                }
                if s == Screen::Content {
                    self.content_view.refresh();
                }
                self.screen = s;
            }
            Target::Search(e) => {
                if self.search.as_ref().map(|s| s.entity) != Some(e) {
                    self.search = Some(SearchView::new(e, &self.prefs));
                }
                self.screen = Screen::Search;
            }
            Target::Editor { entity, id, is_new, copy_from } => {
                if !is_new {
                    let route = ENTITIES[entity].route.to_string();
                    let text = display(&id);
                    let list = self.prefs.recent.entry(route).or_default();
                    list.retain(|x| *x != text);
                    list.insert(0, text);
                    list.truncate(12);
                }
                if self.search.as_ref().map(|s| s.entity) != Some(entity) {
                    self.search = Some(SearchView::new(entity, &self.prefs));
                }
                let ctx = self.ctx.clone();
                self.editor = Some(EditorView::open(&ctx, &db, entity, id, is_new, copy_from));
                self.screen = Screen::Editor;
            }
            Target::Disconnect => {
                self.db = None;
                self.editor = None;
                self.search = None;
                self.refs.clear();
                self.screen = Screen::Connect;
            }
        }
    }

    fn sidebar(&mut self, ui: &mut Ui) -> Option<Target> {
        let mut target = None;
        let db = self.db.clone()?;
        egui::Panel::left("nav").resizable(false).exact_size(236.0).show(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if let Some(e) = &self.emblem {
                    ui.add(egui::Image::new(e).fit_to_exact_size(egui::vec2(36.0, 36.0)));
                }
                ui.label(RichText::new("ArchForge").size(20.0).strong());
            });
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink(false).max_height(ui.available_height() - 70.0).show(ui, |ui| {
                if ui.add(egui::Button::selectable(self.screen == Screen::Dashboard, "Tableau de bord")).clicked() {
                    target = Some(Target::Screen(Screen::Dashboard));
                }
                let current = match self.screen {
                    Screen::Search => self.search.as_ref().map(|s| s.entity),
                    Screen::Editor => self.editor.as_ref().map(|e| e.entity),
                    _ => None,
                };
                let mut group = "";
                for (i, e) in ENTITIES.iter().enumerate() {
                    if e.group != group {
                        group = e.group;
                        ui.add_space(6.0);
                        ui.label(muted(group.to_uppercase()).small());
                    }
                    if ui.add(egui::Button::selectable(current == Some(i), e.label)).clicked() {
                        target = Some(Target::Search(i));
                    }
                }
                ui.add_space(6.0);
                ui.label(muted("OUTILS").small());
                if ui.add(egui::Button::selectable(self.screen == Screen::Sql, "Éditeur SQL")).clicked() {
                    target = Some(Target::Screen(Screen::Sql));
                }
                if ui.add(egui::Button::selectable(self.screen == Screen::Content, "Couche de contenu")).clicked() {
                    target = Some(Target::Screen(Screen::Content));
                }
            });
            ui.separator();
            pages::connected_label(ui, &db);
            ui.label(muted(&db.settings.world).small());
            if ui.small_button("Déconnexion").clicked() {
                target = Some(Target::Disconnect);
            }
        });
        target
    }

    fn json_dialog(&mut self, ctx: &egui::Context) {
        let Some(mut dialog) = self.modals.json.take() else { return };
        let mut nav = None;
        let mut done = None;
        let resp = egui::Modal::new(egui::Id::unique("json-dialog")).show(ctx, |ui| {
            ui.set_width(900.0);
            ui.heading(&dialog.title);
            egui::ScrollArea::vertical().max_height(560.0).show(ui, |ui| {
                let mut env = Env { refs: &mut self.refs, modals: &mut self.modals, nav: &mut nav };
                widgets::widget(ui, &mut env, &dialog.field, &mut dialog.value, dialog.target.with("dialog"));
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add(primary("Valider")).clicked() {
                    done = Some(true);
                }
                if ui.button("Annuler").clicked() {
                    done = Some(false);
                }
            });
        });
        match done {
            Some(true) => {
                self.modals.results.insert(dialog.target, dialog.value);
            }
            Some(false) => {}
            None if resp.should_close() && !self.modals.any_open() => {}
            None => self.modals.json = Some(dialog),
        }
    }

    fn confirm_dialog(&mut self, ctx: &egui::Context) {
        let Some(confirm) = self.modals.confirm.take() else { return };
        let mut answer = None;
        let resp = egui::Modal::new(egui::Id::unique("confirm")).show(ctx, |ui| {
            ui.set_width(420.0);
            ui.heading(&confirm.title);
            ui.label(&confirm.message);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add(egui::Button::new(RichText::new(&confirm.ok).strong()).fill(common::ERR)).clicked() {
                    answer = Some(true);
                }
                if ui.button("Annuler").clicked() {
                    answer = Some(false);
                }
            });
        });
        match answer {
            Some(true) => match confirm.action {
                ConfirmAction::Leave => {
                    if let Some(t) = self.pending.take() {
                        self.editor = None;
                        self.apply(t);
                    }
                }
                ConfirmAction::Reload => {
                    if let (Some(e), Some(db)) = (&mut self.editor, &self.db) {
                        e.reload(ctx, db);
                    }
                }
                ConfirmAction::CloseApp => {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            },
            Some(false) => self.pending = None,
            None if resp.should_close() => self.pending = None,
            None => self.modals.confirm = Some(confirm),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Fermeture de la fenêtre : confirmation s'il reste des modifications.
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.editor_dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.modals.confirm = Some(Confirm {
                title: "Fermer ArchForge".into(),
                message: "Des modifications ne sont pas enregistrées. Fermer quand même ?".into(),
                ok: "Fermer".into(),
                action: ConfirmAction::CloseApp,
            });
        }
        if let Some(db) = &self.db {
            self.refs.pump(&ctx, db);
        }

        let mut target = self.sidebar(ui);
        let mut nav: Option<Nav> = None;
        let mut content_request = None;

        // Sans connexion, l'image de fond occupe toute la fenêtre : pas de marge.
        let panel = if self.db.is_some() { egui::CentralPanel::default() } else { egui::CentralPanel::no_frame() };
        panel.show(ui, |ui| {
            let Some(db) = self.db.clone() else {
                if let Some(db) = self.connect.show(ui, self.logo.as_ref(), self.background.as_ref(), self.archcore.as_ref(), &mut self.prefs) {
                    self.toasts.ok(format!("Connecté à MariaDB {}", db.version));
                    self.db = Some(Arc::new(db));
                    self.dashboard.refresh();
                    self.screen = Screen::Dashboard;
                    if let Some(open) = self.startup.open.take() {
                        let mut parts = open.splitn(3, '/');
                        let (route, id) = (parts.next().unwrap_or_default(), parts.next());
                        self.open_tab = parts.next().map(str::to_string);
                        if let Some(entity) = by_route(route) {
                            target = Some(match id.and_then(|i| ENTITIES[entity].parse_id(i)) {
                                Some(id) => Target::Editor { entity, id, is_new: false, copy_from: None },
                                None => Target::Search(entity),
                            });
                        }
                    }
                }
                return;
            };
            let mut env = Env { refs: &mut self.refs, modals: &mut self.modals, nav: &mut nav };
            match self.screen {
                Screen::Connect | Screen::Dashboard => {
                    if let Some(DashboardAction::Open(i)) = self.dashboard.show(ui, &db, self.content.as_ref()) {
                        target = Some(Target::Search(i));
                    }
                }
                Screen::Search => {
                    if let Some(view) = &mut self.search {
                        if let Some(open) = view.show(ui, &mut env, &db, &mut self.prefs, &mut self.toasts) {
                            target = Some(Target::Editor { entity: view.entity, id: open.id, is_new: open.is_new, copy_from: open.copy_from });
                        }
                    }
                }
                Screen::Editor => {
                    if let Some(view) = &mut self.editor {
                        if let Some(EditorRequest::Content { title, sql, full }) = view.show(ui, &mut env, &db, &mut self.toasts) {
                            content_request = Some((title, sql, full));
                        }
                    }
                }
                Screen::Sql => self.sql.show(ui, &db, &mut self.prefs),
                Screen::Content => self.content_view.show(ui, &mut self.content, &mut self.prefs, &mut self.toasts),
            }
        });

        if let Some((title, sql, full)) = content_request {
            self.content_dialog = Some(ContentDialog::new(title, sql, full, self.content.as_ref()));
        }
        if let Some(n) = nav {
            target = by_route(n.route).map(|entity| match n.id {
                Value::Null => Target::Search(entity),
                id => Target::Editor { entity, id, is_new: false, copy_from: None },
            });
        }
        if let Some(t) = target {
            self.go(t);
            if let (Some(tab), Some(editor)) = (self.open_tab.take(), &mut self.editor) {
                editor.select_tab(&tab);
            }
        }

        // Fenêtres.
        self.modals.show_simple(&ctx, self.db.as_ref());
        self.json_dialog(&ctx);
        self.confirm_dialog(&ctx);
        if let Some(dialog) = &mut self.content_dialog {
            if dialog.show(&ctx, self.content.as_ref(), &mut self.toasts) {
                self.content_dialog = None;
                self.content_view.refresh();
            }
        }
        self.toasts.show(&ctx);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.prefs);
    }
}
