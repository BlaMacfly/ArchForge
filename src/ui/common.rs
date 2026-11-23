//! Outils communs de l'interface : thème « forge », coloration du SQL, notifications, noms des entrées
//! référencées (chargés en arrière-plan, par lots), fenêtres modales.

use egui::{Color32, RichText, Ui};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::db::Db;
use crate::model::{ref_def, Field, RefKind};
use crate::task::Task;

pub const ACCENT: Color32 = Color32::from_rgb(255, 138, 42);
pub const ACCENT_LIGHT: Color32 = Color32::from_rgb(255, 179, 92);
pub const OK: Color32 = Color32::from_rgb(92, 201, 138);
pub const WARN: Color32 = Color32::from_rgb(232, 182, 76);
pub const ERR: Color32 = Color32::from_rgb(255, 107, 94);
pub const MUTED: Color32 = Color32::from_rgb(154, 149, 141);

pub fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = Color32::from_rgb(24, 24, 28);
    visuals.window_fill = Color32::from_rgb(30, 30, 35);
    visuals.extreme_bg_color = Color32::from_rgb(14, 14, 16);
    visuals.faint_bg_color = Color32::from_rgb(32, 32, 37);
    visuals.selection.bg_fill = Color32::from_rgb(140, 70, 15);
    visuals.selection.stroke.color = ACCENT_LIGHT;
    visuals.hyperlink_color = ACCENT_LIGHT;
    visuals.widgets.hovered.bg_stroke.color = ACCENT;
    visuals.widgets.active.bg_fill = Color32::from_rgb(120, 60, 12);
    ctx.set_visuals(visuals);
    ctx.global_style_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
    });
}

pub fn muted(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(MUTED)
}

pub fn error(text: impl Into<String>) -> RichText {
    RichText::new(text.into()).color(ERR)
}

/// Bouton principal (orange).
pub fn primary(text: &str) -> egui::Button<'static> {
    egui::Button::new(RichText::new(text.to_string()).strong().color(Color32::from_rgb(26, 15, 5))).fill(ACCENT)
}

/// Carte : cadre arrondi autour d'un bloc.
pub fn card<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    egui::Frame::new()
        .fill(Color32::from_rgba_unmultiplied(30, 30, 35, 236))
        .stroke(egui::Stroke::new(1.0, Color32::from_rgb(52, 52, 59)))
        .corner_radius(8.0)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

// ------------------------------------------------------------------ SQL coloré

const KEYWORDS: &[&str] = &[
    "SELECT", "FROM", "WHERE", "AND", "OR", "NOT", "NULL", "IS", "IN", "INSERT", "INTO", "VALUES", "UPDATE", "SET", "DELETE", "ON",
    "DUPLICATE", "KEY", "ORDER", "BY", "GROUP", "LIMIT", "OFFSET", "JOIN", "LEFT", "AS", "DISTINCT", "COUNT", "MAX", "MIN", "COALESCE",
    "LIKE", "CASE", "WHEN", "THEN", "ELSE", "END", "CREATE", "TABLE", "ALTER", "DROP", "START", "TRANSACTION", "COMMIT", "ROLLBACK",
    "USE", "SHOW", "DESC", "ASC", "HAVING", "UNION", "ALL", "EXISTS", "JSON_SET", "JSON_REMOVE", "JSON_EXTRACT", "JSON_VALUE", "IF",
];

pub fn sql_job(sql: &str, wrap_width: f32) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};
    let font = egui::FontId::monospace(12.5);
    let fmt = |color: Color32| TextFormat { font_id: font.clone(), color, ..Default::default() };
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    let chars: Vec<char> = sql.chars().collect();
    let mut i = 0;
    let plain = Color32::from_rgb(220, 216, 210);
    while i < chars.len() {
        let c = chars[i];
        let start = i;
        let color;
        if c == '-' && chars.get(i + 1) == Some(&'-') || c == '#' {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            color = MUTED;
        } else if c == '\'' || c == '`' || c == '"' {
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i += 2;
                    continue;
                }
                if chars[i] == c {
                    i += 1;
                    break;
                }
                i += 1;
            }
            color = if c == '\'' { Color32::from_rgb(159, 212, 134) } else { Color32::from_rgb(226, 192, 141) };
        } else if c.is_ascii_digit() {
            while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == '.') {
                i += 1;
            }
            color = Color32::from_rgb(121, 192, 255);
        } else if c.is_alphabetic() || c == '_' {
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            color = if KEYWORDS.contains(&word.to_uppercase().as_str()) { Color32::from_rgb(255, 157, 77) } else { plain };
        } else {
            i += 1;
            color = plain;
        }
        let i = i.min(chars.len());
        job.append(&chars[start..i].iter().collect::<String>(), 0.0, fmt(color));
    }
    job
}

// ------------------------------------------------------------------ notifications

pub struct Toasts {
    items: Vec<(String, Color32, Instant)>,
}

impl Default for Toasts {
    fn default() -> Self {
        Toasts { items: Vec::new() }
    }
}

impl Toasts {
    pub fn ok(&mut self, text: impl Into<String>) {
        self.items.push((text.into(), OK, Instant::now()));
    }
    pub fn error(&mut self, text: impl Into<String>) {
        self.items.push((text.into(), ERR, Instant::now()));
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        let life = |c: Color32| if c == ERR { Duration::from_secs(10) } else { Duration::from_secs(5) };
        self.items.retain(|(_, c, t)| t.elapsed() < life(*c));
        if self.items.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::unique("toasts")).anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-16.0, -16.0)).show(ctx, |ui| {
            ui.set_max_width(440.0);
            for (text, color, _) in &self.items {
                egui::Frame::new()
                    .fill(Color32::from_rgb(38, 38, 44))
                    .stroke(egui::Stroke::new(1.0, *color))
                    .corner_radius(6.0)
                    .inner_margin(10.0)
                    .show(ui, |ui| {
                        ui.label(RichText::new(text).color(Color32::from_rgb(233, 230, 225)));
                    });
            }
        });
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

// ------------------------------------------------------------------ noms des références

type Lookup = Vec<(RefKind, String, Option<String>)>;

/// Noms des entrées référencées (« Hydra's Heart » à côté du TID 964), demandés par lots.
#[derive(Default)]
pub struct RefCache {
    names: HashMap<(RefKind, String), Option<String>>,
    pending: HashSet<(RefKind, String)>,
    inflight: Vec<Task<Lookup>>,
}

pub fn id_text(v: &Value) -> Option<String> {
    match v {
        Value::Null => None,
        Value::String(s) if s.trim().is_empty() => None,
        Value::String(s) => Some(s.trim().to_string()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

impl RefCache {
    /// Nom connu (Some(Some)), introuvable (Some(None)), ou pas encore chargé (None).
    pub fn name(&mut self, kind: RefKind, id: &Value) -> Option<Option<String>> {
        let key = (kind, id_text(id)?);
        if let Some(n) = self.names.get(&key) {
            return Some(n.clone());
        }
        self.pending.insert(key);
        None
    }

    pub fn forget(&mut self, kind: RefKind, id: &Value) {
        if let Some(t) = id_text(id) {
            self.names.remove(&(kind, t));
        }
    }

    pub fn clear(&mut self) {
        self.names.clear();
    }

    /// Envoie les demandes en attente et range les réponses arrivées.
    pub fn pump(&mut self, ctx: &egui::Context, db: &Arc<Db>) {
        self.inflight.retain(|t| match t.poll() {
            Some(Ok(found)) => {
                for (k, id, name) in found {
                    self.names.insert((k, id), name);
                }
                false
            }
            Some(Err(_)) => false,
            None => true,
        });
        if self.pending.is_empty() {
            return;
        }
        let mut by_kind: HashMap<RefKind, Vec<String>> = HashMap::new();
        for (k, id) in self.pending.drain() {
            // Marque « en cours » le temps de la requête : évite de redemander à chaque image.
            self.names.insert((k, id.clone()), Some(String::from("…")));
            by_kind.entry(k).or_default().push(id);
        }
        for (kind, ids) in by_kind {
            let db = db.clone();
            self.inflight.push(Task::spawn(ctx, move || {
                let r = ref_def(kind);
                let marks = vec!["?"; ids.len()].join(", ");
                let group = if r.grouped { format!(" GROUP BY {}", r.key) } else { String::new() };
                let sql = format!("SELECT {} AS k, {} AS n FROM {} WHERE {} IN ({marks}){group}", r.key, r.name, r.from, r.key);
                let params: Vec<Value> = ids
                    .iter()
                    .map(|i| if r.text { Value::String(i.clone()) } else { i.parse::<i64>().map(Value::from).unwrap_or(Value::String(i.clone())) })
                    .collect();
                let rows = db.select_maps(r.db, &sql, &params).unwrap_or_default();
                let found: HashMap<String, String> = rows
                    .into_iter()
                    .filter_map(|row| Some((id_text(row.get("k")?)?, crate::editor::display(row.get("n")?))))
                    .collect();
                Ok(ids.into_iter().map(|id| (kind, id.clone(), found.get(&id).cloned())).collect())
            }));
        }
    }
}

// ------------------------------------------------------------------ fenêtres modales

/// Fenêtre de recherche d'une entrée référencée (les « selectors » de Keira3).
pub struct Picker {
    pub target: egui::Id,
    pub kind: RefKind,
    pub query: String,
    pub searched: Option<String>,
    pub task: Option<Task<Vec<crate::sql::Row>>>,
    pub rows: Vec<crate::sql::Row>,
    pub error: Option<String>,
}

pub struct FlagsDialog {
    pub target: egui::Id,
    pub title: String,
    pub help: String,
    pub bits: &'static [(i64, &'static str)],
    pub value: i64,
}

pub struct JsonDialog {
    pub target: egui::Id,
    pub title: String,
    pub field: Field,
    pub value: Value,
}

#[derive(Clone, Copy, PartialEq)]
pub enum ConfirmAction {
    /// Quitter l'entrée en cours (navigation en attente).
    Leave,
    Reload,
    CloseApp,
}

pub struct Confirm {
    pub title: String,
    pub message: String,
    pub ok: String,
    pub action: ConfirmAction,
}

#[derive(Default)]
pub struct Modals {
    pub picker: Option<Picker>,
    pub flags: Option<FlagsDialog>,
    pub json: Option<JsonDialog>,
    pub confirm: Option<Confirm>,
    /// Valeurs choisies dans une fenêtre, à reprendre par le champ qui l'a ouverte (même identifiant).
    pub results: HashMap<egui::Id, Value>,
}

impl Modals {
    pub fn open_picker(&mut self, target: egui::Id, kind: RefKind) {
        self.picker = Some(Picker { target, kind, query: String::new(), searched: None, task: None, rows: Vec::new(), error: None });
    }

    pub fn take_result(&mut self, id: egui::Id) -> Option<Value> {
        self.results.remove(&id)
    }

    pub fn any_open(&self) -> bool {
        self.picker.is_some() || self.flags.is_some() || self.json.is_some() || self.confirm.is_some()
    }

    /// Fenêtres simples (sélecteur, drapeaux) ; la confirmation et l'édition JSON sont dessinées par l'application.
    pub fn show_simple(&mut self, ctx: &egui::Context, db: Option<&Arc<Db>>) {
        if let Some(p) = &mut self.picker {
            let mut chosen = None;
            let mut close = false;
            let r = ref_def(p.kind);
            if p.searched.as_deref() != Some(p.query.as_str()) && p.task.is_none() {
                if let Some(db) = db {
                    let db = db.clone();
                    let term = p.query.trim().to_string();
                    p.searched = Some(p.query.clone());
                    p.task = Some(Task::spawn(ctx, move || {
                        let mut sql = format!("SELECT {} AS k, {} AS n FROM {}", r.key, r.name, r.from);
                        let mut params = Vec::new();
                        if !term.is_empty() && !r.text && term.parse::<i64>().is_ok() {
                            sql.push_str(&format!(" WHERE {} = ?", r.key));
                            params.push(Value::from(term.parse::<i64>().unwrap_or(0)));
                        } else if !term.is_empty() && !r.grouped {
                            sql.push_str(&format!(" WHERE {} LIKE ?", r.name));
                            params.push(Value::String(format!("%{term}%")));
                        }
                        if r.grouped {
                            sql.push_str(&format!(" GROUP BY {}", r.key));
                        }
                        sql.push_str(&format!(" ORDER BY {} LIMIT 200", r.key));
                        db.select_maps(r.db, &sql, &params)
                    }));
                }
            }
            if let Some(t) = &p.task {
                if let Some(result) = t.poll() {
                    p.task = None;
                    match result {
                        Ok(rows) => {
                            p.rows = rows;
                            p.error = None;
                        }
                        Err(e) => p.error = Some(format!("{e:#}")),
                    }
                }
            }
            let resp = egui::Modal::new(egui::Id::unique("picker")).show(ctx, |ui| {
                ui.set_width(620.0);
                ui.heading(format!("Choisir : {}", r.label));
                let edit = ui.add(egui::TextEdit::singleline(&mut p.query).hint_text(if r.text { "Nom…" } else { "TID ou nom…" }).desired_width(f32::INFINITY));
                edit.request_focus();
                if let Some(e) = &p.error {
                    ui.label(error(e));
                }
                egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                    if p.rows.is_empty() {
                        ui.label(muted(if p.task.is_some() { "Recherche…" } else { "Aucun résultat." }));
                    }
                    for row in &p.rows {
                        let k = row.get("k").cloned().unwrap_or(Value::Null);
                        let n = crate::editor::display(row.get("n").unwrap_or(&Value::Null));
                        let text = if r.text { n.clone() } else { format!("{}   {n}", crate::editor::display(&k)) };
                        if ui.add(egui::Button::selectable(false, text)).clicked() {
                            chosen = Some(k);
                        }
                    }
                });
                ui.separator();
                if ui.button("Annuler").clicked() {
                    close = true;
                }
            });
            if let Some(v) = chosen {
                self.results.insert(p.target, v);
                close = true;
            }
            if close || resp.should_close() {
                self.picker = None;
            }
        }

        if let Some(fl) = &mut self.flags {
            let mut done = None;
            let resp = egui::Modal::new(egui::Id::unique("flags")).show(ctx, |ui| {
                ui.set_width(380.0);
                ui.heading(&fl.title);
                if !fl.help.is_empty() {
                    ui.label(muted(&fl.help));
                }
                for (bit, label) in fl.bits {
                    let mut on = fl.value & bit != 0;
                    if ui.checkbox(&mut on, format!("{label}  ({bit})")).changed() {
                        fl.value = if on { fl.value | bit } else { fl.value & !bit };
                    }
                }
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
            if done == Some(true) {
                self.results.insert(fl.target, Value::from(fl.value));
            }
            if done.is_some() || resp.should_close() {
                self.flags = None;
            }
        }
    }
}
