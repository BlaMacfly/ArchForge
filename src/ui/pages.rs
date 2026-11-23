//! Pages hors éditeurs : connexion, tableau de bord, éditeur SQL, couche de contenu, et fenêtre d'ajout à la
//! couche de contenu.

use egui::{RichText, Ui};
use std::path::PathBuf;
use std::sync::Arc;

use super::common::{card, error, muted, primary, sql_job, Toasts, ACCENT_LIGHT, OK, WARN};
use super::Prefs;
use crate::content::{forbidden_statements, ContentDir, ContentFile};
use crate::db::{BaseInfo, Db, ResultSet, Settings, ALIASES};
use crate::defs::ENTITIES;
use crate::editor::display;
use crate::task::Slot;

// ------------------------------------------------------------------ connexion

pub struct ConnectView {
    pub settings: Settings,
    pub remember: bool,
    task: Slot<Db>,
    error: Option<String>,
}

impl ConnectView {
    pub fn new(defaults: Settings, prefs: &Prefs) -> Self {
        let settings = prefs.connections.first().cloned().unwrap_or(defaults.clone());
        let settings = if settings.password.is_empty() { Settings { password: defaults.password.clone(), ..settings } } else { settings };
        ConnectView { remember: prefs.remember_password, settings, task: Slot::default(), error: None }
    }

    /// Lance la connexion avec les réglages affichés.
    pub fn start(&mut self, ctx: &egui::Context) {
        if self.task.busy() {
            return;
        }
        self.error = None;
        let settings = self.settings.clone();
        self.task.start(ctx, move || Db::connect(settings));
    }

    /// Renvoie la connexion établie.
    pub fn show(
        &mut self,
        ui: &mut Ui,
        logo: Option<&egui::TextureHandle>,
        background: Option<&egui::TextureHandle>,
        archcore: Option<&PathBuf>,
        prefs: &mut Prefs,
    ) -> Option<Db> {
        let mut connected = None;
        // Fond : l'image couvre toute la page (recadrée au centre), sous un voile qui garde le formulaire lisible.
        if let Some(bg) = background {
            let rect = ui.max_rect();
            let size = bg.size_vec2();
            let shown = size * (rect.width() / size.x).max(rect.height() / size.y);
            let uv = egui::Rect::from_center_size(egui::pos2(0.5, 0.5), egui::vec2(rect.width() / shown.x, rect.height() / shown.y));
            ui.painter().image(bg.id(), rect, uv, egui::Color32::WHITE);
            ui.painter().rect_filled(rect, 0.0, egui::Color32::from_black_alpha(110));
        }
        if let Some(r) = self.task.poll() {
            match r {
                Ok(db) => {
                    let mut saved = self.settings.clone();
                    if !self.remember {
                        saved.password.clear();
                    }
                    prefs.connections.retain(|c| !(c.host == saved.host && c.port == saved.port && c.user == saved.user && c.world == saved.world));
                    prefs.connections.insert(0, saved);
                    prefs.connections.truncate(6);
                    prefs.remember_password = self.remember;
                    connected = Some(db);
                }
                Err(e) => self.error = Some(format!("{e:#}")),
            }
        }
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            ui.vertical_centered(|ui| {
                if let Some(logo) = logo {
                    ui.add(egui::Image::new(logo).max_width(260.0));
                }
                ui.label(muted("Éditeur de la base de données d'ArchCore"));
            });
            ui.add_space(10.0);
            ui.vertical_centered(|ui| {
                ui.set_max_width(720.0);
                card(ui, |ui| ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    ui.label(RichText::new("Connexion à MariaDB").strong());
                    match archcore {
                        Some(dir) => ui.label(muted(format!("Valeurs lues dans config/world.conf de {}", dir.display()))),
                        None => ui.label(muted("Lancer ArchForge avec --archcore <dépôt ArchCore>, ou depuis ce dossier, pour reprendre sa configuration et activer la couche de contenu.")),
                    };
                    let s = &mut self.settings;
                    let mut port = s.port.to_string();
                    // Deux champs par ligne, libellés de largeur fixe.
                    let field = |ui: &mut Ui, label: &str, edit: egui::TextEdit| {
                        ui.add_sized([140.0, 22.0], egui::Label::new(label));
                        ui.add(edit.desired_width(170.0))
                    };
                    ui.horizontal(|ui| {
                        field(ui, "Hôte", egui::TextEdit::singleline(&mut s.host));
                        if field(ui, "Port", egui::TextEdit::singleline(&mut port)).changed() {
                            s.port = port.trim().parse().unwrap_or(s.port);
                        }
                    });
                    ui.horizontal(|ui| {
                        field(ui, "Utilisateur", egui::TextEdit::singleline(&mut s.user));
                        field(ui, "Mot de passe", egui::TextEdit::singleline(&mut s.password).password(true));
                    });
                    ui.horizontal(|ui| {
                        field(ui, "Base monde", egui::TextEdit::singleline(&mut s.world));
                        field(ui, "Base des comptes", egui::TextEdit::singleline(&mut s.auth));
                    });
                    ui.horizontal(|ui| {
                        field(ui, "Base des personnages", egui::TextEdit::singleline(&mut s.characters));
                        field(ui, "Base des journaux", egui::TextEdit::singleline(&mut s.logs));
                    });
                    ui.checkbox(&mut self.remember, "Mémoriser le mot de passe sur ce poste");
                    if let Some(e) = &self.error {
                        ui.label(error(e));
                    }
                    let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
                    ui.horizontal(|ui| {
                        if ui.add_enabled(!self.task.busy(), primary("Se connecter")).clicked() || enter {
                            self.start(ui.ctx());
                        }
                        if self.task.busy() {
                            ui.spinner();
                        }
                    });
                }));
                if !prefs.connections.is_empty() {
                    ui.add_space(6.0);
                    card(ui, |ui| {
                        ui.label(RichText::new("Connexions récentes").strong());
                        ui.horizontal_wrapped(|ui| {
                            for c in prefs.connections.clone() {
                                if ui.button(format!("{}@{}:{} — {}", c.user, c.host, c.port, c.world)).clicked() {
                                    let password = if c.password.is_empty() { self.settings.password.clone() } else { c.password.clone() };
                                    self.settings = Settings { password, ..c };
                                }
                            }
                        });
                    });
                }
            });
        });
        connected
    }
}

// ------------------------------------------------------------------ tableau de bord

#[derive(Default)]
pub struct DashboardView {
    task: Slot<(Vec<BaseInfo>, Vec<crate::sql::Row>)>,
    data: Option<(Vec<BaseInfo>, Vec<crate::sql::Row>)>,
    error: Option<String>,
    started: bool,
}

pub enum DashboardAction {
    Open(usize),
}

impl DashboardView {
    pub fn refresh(&mut self) {
        self.started = false;
    }

    pub fn show(&mut self, ui: &mut Ui, db: &Arc<Db>, content: Option<&PathBuf>) -> Option<DashboardAction> {
        if !self.started {
            self.started = true;
            let db = db.clone();
            self.task.start(ui.ctx(), move || {
                let bases = db.overview()?;
                let imports = db
                    .select_maps(
                        "world",
                        "SELECT source, files, archcore_version, CAST(finished_at AS CHAR) AS finished FROM import_run ORDER BY id DESC LIMIT 6",
                        &[],
                    )
                    .unwrap_or_default();
                Ok((bases, imports))
            });
        }
        if let Some(r) = self.task.poll() {
            match r {
                Ok(d) => self.data = Some(d),
                Err(e) => self.error = Some(format!("{e:#}")),
            }
        }
        let mut action = None;
        ui.heading(RichText::new("Tableau de bord").strong());
        ui.label(muted(format!("MariaDB {} — {}@{}:{}", db.version, db.settings.user, db.settings.host, db.settings.port)));
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            card(ui, |ui| {
                ui.label(RichText::new("À savoir").strong());
                ui.label("• ArchCore charge le monde au démarrage : redémarrer le serveur pour qu'il voie les modifications.");
                ui.label("• `archcore import` régénère la base monde depuis les fichiers du client : ce qui doit durer va dans la couche de contenu (« Ajouter au contenu »), rejouée après chaque import.");
                ui.label("• Chaque requête est affichée avant de partir et passe dans une transaction : tout ou rien.");
            });
            if let Some(e) = &self.error {
                ui.label(error(e));
            }
            let Some((bases, imports)) = &self.data else {
                ui.spinner();
                return;
            };
            let world = bases.iter().find(|b| b.alias == "world");
            let count = |table: &str| world.and_then(|w| w.tables.iter().find(|(t, _)| t == table)).map(|(_, n)| *n);
            let mut groups: Vec<&str> = Vec::new();
            for e in ENTITIES.iter() {
                if !groups.contains(&e.group) {
                    groups.push(e.group);
                }
            }
            for group in groups {
                ui.add_space(6.0);
                ui.label(RichText::new(group).strong().color(ACCENT_LIGHT));
                ui.horizontal_wrapped(|ui| {
                    for (i, e) in ENTITIES.iter().enumerate().filter(|(_, e)| e.group == group) {
                        let text = match (e.db == "world").then(|| count(e.table)).flatten() {
                            Some(n) => format!("{}\n{n}", e.label),
                            None => e.label.to_string(),
                        };
                        if ui.add_sized([180.0, 44.0], egui::Button::new(text)).clicked() {
                            action = Some(DashboardAction::Open(i));
                        }
                    }
                });
            }
            ui.add_space(8.0);
            ui.columns(2, |cols| {
                card(&mut cols[0], |ui| {
                    ui.label(RichText::new("Bases").strong());
                    egui::Grid::new("bases").striped(true).show(ui, |ui| {
                        for h in ["Rôle", "Base", "Tables", "Migrations"] {
                            ui.label(RichText::new(h).strong());
                        }
                        ui.end_row();
                        for b in bases {
                            let label = ALIASES.iter().find(|(a, _)| *a == b.alias).map(|(_, l)| *l).unwrap_or("");
                            ui.label(label);
                            ui.label(&b.schema);
                            ui.label(if b.exists { b.tables.len().to_string() } else { "absente".into() });
                            ui.label(match &b.last_migration {
                                Some(m) => format!("{} (dernière : {m})", b.migrations),
                                None => "—".into(),
                            });
                            ui.end_row();
                        }
                    });
                });
                card(&mut cols[1], |ui| {
                    ui.label(RichText::new("Couche de contenu et imports").strong());
                    match content {
                        Some(p) => ui.label(muted(p.display().to_string())),
                        None => ui.label(RichText::new("Aucun dossier de contenu : voir la page Couche de contenu.").color(WARN)),
                    };
                    egui::Grid::new("imports").striped(true).show(ui, |ui| {
                        for h in ["Source", "Fichiers", "Version", "Fin"] {
                            ui.label(RichText::new(h).strong());
                        }
                        ui.end_row();
                        for r in imports {
                            for c in ["source", "files", "archcore_version", "finished"] {
                                ui.label(r.get(c).map(display).unwrap_or_default());
                            }
                            ui.end_row();
                        }
                    });
                });
            });
        });
        action
    }
}

// ------------------------------------------------------------------ éditeur SQL

#[derive(Default)]
pub struct SqlView {
    task: Slot<Vec<ResultSet>>,
    results: Option<Result<Vec<ResultSet>, String>>,
    elapsed: Option<std::time::Instant>,
    took: Option<std::time::Duration>,
    pub transaction: bool,
}

impl SqlView {
    pub fn new() -> Self {
        SqlView { transaction: true, ..Default::default() }
    }

    pub fn show(&mut self, ui: &mut Ui, db: &Arc<Db>, prefs: &mut Prefs) {
        if let Some(r) = self.task.poll() {
            self.took = self.elapsed.map(|t| t.elapsed());
            self.results = Some(r.map_err(|e| format!("{e:#}")));
        }
        ui.heading(RichText::new("Éditeur SQL").strong());
        ui.label(muted("Requêtes libres sur une base d'ArchCore. Ctrl+Entrée pour exécuter."));
        if prefs.sql_db.is_empty() {
            prefs.sql_db = "world".into();
        }
        let mut run = false;
        ui.horizontal(|ui| {
            ui.label("Base");
            let current = ALIASES.iter().find(|(a, _)| *a == prefs.sql_db).map(|(_, l)| *l).unwrap_or("Monde");
            egui::ComboBox::from_id_salt("sql-db").selected_text(current).show_ui(ui, |ui| {
                for (a, l) in ALIASES {
                    ui.selectable_value(&mut prefs.sql_db, a.to_string(), l);
                }
            });
            ui.checkbox(&mut self.transaction, "Dans une transaction (tout ou rien)");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add_enabled(!self.task.busy(), primary("Exécuter")).clicked() {
                    run = true;
                }
                if ui.button("Copier").clicked() {
                    ui.ctx().copy_text(prefs.sql_text.clone());
                }
            });
        });
        let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, wrap: f32| ui.fonts_mut(|f| f.layout_job(sql_job(text.as_str(), wrap)));
        let resp = ui.add(
            egui::TextEdit::multiline(&mut prefs.sql_text)
                .code_editor()
                .desired_rows(10)
                .desired_width(f32::INFINITY)
                .layouter(&mut layouter),
        );
        if resp.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.command) {
            run = true;
        }
        if run && !self.task.busy() {
            let (db, alias, sql, tx) = (db.clone(), prefs.sql_db.clone(), prefs.sql_text.clone(), self.transaction);
            self.elapsed = Some(std::time::Instant::now());
            self.task.start(ui.ctx(), move || db.script(&alias, &sql, tx));
        }
        ui.add_space(6.0);
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| match &self.results {
            None if self.task.busy() => {
                ui.spinner();
            }
            None => {}
            Some(Err(e)) => {
                ui.label(error(e));
            }
            Some(Ok(sets)) => {
                ui.label(muted(format!("{} résultat(s) en {} ms", sets.len(), self.took.map(|d| d.as_millis()).unwrap_or(0))));
                for (n, set) in sets.iter().enumerate() {
                    if set.columns.is_empty() {
                        ui.label(format!("{} ligne(s) touchée(s)", set.affected));
                        continue;
                    }
                    egui::Grid::new(("sql-result", n)).striped(true).show(ui, |ui| {
                        for c in &set.columns {
                            ui.label(RichText::new(c).strong());
                        }
                        ui.end_row();
                        for row in &set.rows {
                            for v in row {
                                let text = match crate::db::binary_len(v) {
                                    Some(len) => format!("<binaire, {len} octets>"),
                                    None if v.is_null() => "NULL".into(),
                                    None => display(v).chars().take(160).collect(),
                                };
                                ui.label(text);
                            }
                            ui.end_row();
                        }
                    });
                    if set.truncated {
                        ui.label(RichText::new("Résultat tronqué à 5 000 lignes.").color(WARN));
                    }
                    ui.add_space(6.0);
                }
            }
        });
    }
}

// ------------------------------------------------------------------ couche de contenu

#[derive(Default)]
pub struct ContentView {
    files: Option<Result<Vec<ContentFile>, String>>,
    selected: Option<(String, String)>,
    pub dir_input: String,
}

impl ContentView {
    pub fn refresh(&mut self) {
        self.files = None;
    }

    pub fn show(&mut self, ui: &mut Ui, content: &mut Option<PathBuf>, prefs: &mut Prefs, toasts: &mut Toasts) {
        if self.dir_input.is_empty() {
            if let Some(p) = content {
                self.dir_input = p.display().to_string();
            }
        }
        ui.heading(RichText::new("Couche de contenu").strong());
        ui.label(muted(
            "Fichiers <AAAAMMJJHHMM>_<sujet>.sql du dossier content/ d'ArchCore : rejoués par `archcore import` (à la fin) et `archcore content`, \
             chacun dans une transaction, sur la base monde.",
        ));
        card(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Dossier");
                ui.add(egui::TextEdit::singleline(&mut self.dir_input).desired_width(520.0));
                if ui.button("Parcourir…").clicked() {
                    if let Some(p) = rfd::FileDialog::new().set_title("Dossier content/ d'ArchCore").pick_folder() {
                        self.dir_input = p.display().to_string();
                    }
                }
                if ui.button("Utiliser ce dossier").clicked() {
                    let p = PathBuf::from(self.dir_input.trim());
                    if p.is_dir() {
                        prefs.content_dir = Some(p.display().to_string());
                        *content = Some(p);
                        self.files = None;
                        toasts.ok("Dossier de contenu choisi");
                    } else {
                        toasts.error(format!("{} n'est pas un dossier", p.display()));
                    }
                }
            });
            ui.label(muted("Règles : données seulement (pas de CREATE, ALTER, DROP…), requêtes rejouables, aucune valeur copiée des fichiers du client. Appliquer ensuite avec `archcore content`."));
        });
        let Some(dir) = content.clone() else {
            ui.label(RichText::new("Aucun dossier de contenu.").color(WARN));
            return;
        };
        if self.files.is_none() {
            self.files = Some(ContentDir { path: dir.clone() }.list().map_err(|e| format!("{e:#}")));
        }
        ui.add_space(6.0);
        ui.columns(2, |cols| {
            card(&mut cols[0], |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Fichiers").strong());
                    if ui.small_button("Relire").on_hover_text("Relire le dossier").clicked() {
                        self.files = None;
                    }
                });
                match &self.files {
                    Some(Ok(files)) if files.is_empty() => {
                        ui.label(muted("Aucun fichier pour le moment."));
                    }
                    Some(Ok(files)) => {
                        egui::ScrollArea::vertical().id_salt("content-files").max_height(560.0).show(ui, |ui| {
                            for file in files {
                                let selected = self.selected.as_ref().is_some_and(|(n, _)| n == &file.name);
                                if ui.add(egui::Button::selectable(selected, format!("{}   ({} o)", file.name, file.size))).clicked() {
                                    let text = ContentDir { path: dir.clone() }.read(&file.name).unwrap_or_else(|e| format!("-- {e:#}"));
                                    self.selected = Some((file.name.clone(), text));
                                }
                            }
                        });
                    }
                    Some(Err(e)) => {
                        ui.label(error(e));
                    }
                    None => {}
                }
            });
            card(&mut cols[1], |ui| match &self.selected {
                Some((_, text)) => {
                    egui::ScrollArea::vertical().id_salt("content-text").max_height(560.0).show(ui, |ui| {
                        ui.add(egui::Label::new(sql_job(text, ui.available_width())).selectable(true));
                    });
                }
                None => {
                    ui.label(muted("Choisir un fichier."));
                }
            });
        });
    }
}

// ------------------------------------------------------------------ ajout à la couche de contenu

pub struct ContentDialog {
    pub title: String,
    pub sql: String,
    pub full: bool,
    files: Vec<ContentFile>,
    file: Option<String>,
    subject: String,
    comment: String,
    error: Option<String>,
}

impl ContentDialog {
    pub fn new(title: String, sql: String, full: bool, dir: Option<&PathBuf>) -> Self {
        let files = dir.map(|d| ContentDir { path: d.clone() }.list().unwrap_or_default()).unwrap_or_default();
        ContentDialog { subject: title.clone(), comment: title.clone(), title, sql, full, files, file: None, error: None }
    }

    /// Vrai quand la fenêtre se ferme.
    pub fn show(&mut self, ctx: &egui::Context, dir: Option<&PathBuf>, toasts: &mut Toasts) -> bool {
        let mut close = false;
        let forbidden = forbidden_statements(&self.sql);
        let resp = egui::Modal::new(egui::Id::unique("content-dialog")).show(ctx, |ui| {
            ui.set_width(880.0);
            ui.heading("Ajouter à la couche de contenu");
            ui.label(muted("Les fichiers de content/ sont rejoués par `archcore import` et `archcore content`, chacun dans une transaction."));
            if self.full {
                ui.label(RichText::new(
                    "Entrée complète : elle recopie toutes les valeurs, y compris celles importées du client. La couche de contenu ne doit contenir \
                     que des valeurs créées pour ArchCore ; préférer « Changements » pour une entrée importée.",
                ).color(WARN));
            }
            if !forbidden.is_empty() {
                ui.label(error(format!("Refusé par la couche de contenu : {}.", forbidden.join(", "))));
            }
            let Some(dir) = dir else {
                ui.label(error("Aucun dossier de contenu : lancer ArchForge avec --archcore <dépôt ArchCore>, ou en choisir un dans la page Couche de contenu."));
                if ui.button("Fermer").clicked() {
                    close = true;
                }
                return;
            };
            egui::Grid::new("content-form").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
                ui.label("Fichier");
                let label = self.file.clone().unwrap_or_else(|| "— Nouveau fichier —".into());
                egui::ComboBox::from_id_salt("content-file").width(420.0).selected_text(label).show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.file, None, "— Nouveau fichier —");
                    for f in &self.files {
                        ui.selectable_value(&mut self.file, Some(f.name.clone()), &f.name);
                    }
                });
                ui.end_row();
                ui.label("Sujet (nouveau fichier)");
                ui.add_enabled(self.file.is_none(), egui::TextEdit::singleline(&mut self.subject).desired_width(420.0));
                ui.end_row();
                ui.label("Commentaire du bloc");
                ui.add(egui::TextEdit::singleline(&mut self.comment).desired_width(420.0));
                ui.end_row();
            });
            egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                egui::Frame::new().fill(egui::Color32::from_rgb(14, 14, 16)).inner_margin(8.0).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(egui::Label::new(sql_job(&self.sql, ui.available_width())).selectable(true));
                });
            });
            if let Some(e) = &self.error {
                ui.label(error(e));
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.add_enabled(forbidden.is_empty(), primary("Ajouter")).clicked() {
                    let comment = if self.comment.trim().is_empty() { self.title.clone() } else { self.comment.clone() };
                    match (ContentDir { path: dir.clone() }).append(self.file.as_deref(), Some(&self.subject), &comment, &self.sql) {
                        Ok(name) => {
                            toasts.ok(format!("Ajouté à content/{name}"));
                            close = true;
                        }
                        Err(e) => self.error = Some(format!("{e:#}")),
                    }
                }
                if ui.button("Annuler").clicked() {
                    close = true;
                }
            });
        });
        close || resp.should_close()
    }
}

/// Coche verte « Connecté ».
pub fn connected_label(ui: &mut Ui, db: &Db) {
    ui.label(RichText::new(format!("{}@{}", db.settings.user, db.settings.host)).color(OK).small());
}
