//! Page de recherche d'un éditeur : création d'une entrée (identifiant libre proposé, copie possible),
//! entrées récentes, filtres et résultats paginés.

use egui::{RichText, Ui};
use serde_json::Value;
use std::sync::Arc;

use super::common::{card, error, muted, primary, Toasts, ACCENT_LIGHT};
use super::widgets::Env;
use super::Prefs;
use crate::db::{Db, ResultSet};
use crate::defs::ENTITIES;
use crate::editor::{display, search_sql};
use crate::model::{Create, Entity};
use crate::task::Slot;

const PAGE: usize = 50;

/// Entrée à ouvrir.
pub struct Open {
    pub id: Value,
    pub is_new: bool,
    pub copy_from: Option<Value>,
}

pub struct SearchView {
    pub entity: usize,
    filters: Vec<String>,
    page: usize,
    results: Option<ResultSet>,
    task: Slot<ResultSet>,
    stale: bool,
    error: Option<String>,
    create_id: String,
    copy_from: String,
    next_id: Slot<String>,
    exists: Slot<(Value, Option<Value>, bool)>,
}

impl SearchView {
    pub fn new(entity: usize, prefs: &Prefs) -> Self {
        let e = &ENTITIES[entity];
        let mut filters = prefs.filters.get(e.route).cloned().unwrap_or_default();
        filters.resize(e.search.filters.len(), String::new());
        SearchView {
            entity,
            filters,
            page: 0,
            results: None,
            task: Slot::default(),
            stale: true,
            error: None,
            create_id: String::new(),
            copy_from: String::new(),
            next_id: Slot::default(),
            exists: Slot::default(),
        }
    }

    fn entity(&self) -> &'static Entity {
        &ENTITIES[self.entity]
    }

    fn search(&mut self, ctx: &egui::Context, db: &Arc<Db>) {
        let entity = self.entity();
        let active: Vec<(usize, String)> = self.filters.iter().enumerate().filter(|(_, v)| !v.trim().is_empty()).map(|(i, v)| (i, v.trim().to_string())).collect();
        let (sql, params) = search_sql(entity, &active, self.page, PAGE);
        let db = db.clone();
        self.task.start(ctx, move || db.select(entity.db, &sql, &params));
        self.stale = false;
    }

    pub fn show(&mut self, ui: &mut Ui, env: &mut Env, db: &Arc<Db>, prefs: &mut Prefs, toasts: &mut Toasts) -> Option<Open> {
        let ctx = ui.ctx().clone();
        let entity = self.entity();
        let mut open = None;
        if self.stale && !self.task.busy() {
            self.search(&ctx, db);
            if let Create::NextId(q) = entity.create {
                let db = db.clone();
                self.next_id.start(&ctx, move || Ok(db.select_maps(entity.db, q, &[])?.first().and_then(|r| r.get("n")).map(display).unwrap_or_default()));
            }
        }
        if let Some(r) = self.task.poll() {
            match r {
                Ok(set) => {
                    self.results = Some(set);
                    self.error = None;
                }
                Err(e) => self.error = Some(format!("{e:#}")),
            }
        }
        if let Some(Ok(n)) = self.next_id.poll() {
            if self.create_id.is_empty() {
                self.create_id = n;
            }
        }
        if let Some(r) = self.exists.poll() {
            match r {
                Ok((id, copy, false)) => open = Some(Open { id, is_new: true, copy_from: copy }),
                Ok((id, _, true)) => toasts.error(format!("{} {} existe déjà", entity.singular, display(&id))),
                Err(e) => toasts.error(format!("{e:#}")),
            }
        }

        ui.heading(RichText::new(entity.label).strong());
        if !entity.help.is_empty() {
            ui.label(muted(entity.help));
        }
        ui.add_space(6.0);
        egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
            if !matches!(entity.create, Create::No) {
                card(ui, |ui| {
                    ui.label(RichText::new("Nouvelle entrée").strong());
                    ui.horizontal(|ui| {
                        ui.label(entity.key_label);
                        ui.add(egui::TextEdit::singleline(&mut self.create_id).desired_width(160.0));
                        ui.label("Copier depuis");
                        ui.add(egui::TextEdit::singleline(&mut self.copy_from).hint_text("facultatif").desired_width(120.0));
                        let pick_id = egui::Id::unique(("copy-from", self.entity));
                        if let Some(kind) = entity.reference {
                            if ui.small_button("…").on_hover_text("Choisir l'entrée à copier").clicked() {
                                env.modals.open_picker(pick_id, kind);
                            }
                        }
                        if let Some(v) = env.modals.take_result(pick_id) {
                            self.copy_from = display(&v);
                        }
                        if ui.add_enabled(!self.exists.busy(), primary("Créer")).clicked() {
                            match entity.parse_id(&self.create_id) {
                                None => toasts.error(if entity.key_text { "Indiquer un identifiant" } else { "Identifiant numérique attendu" }),
                                Some(id) => {
                                    let copy = if self.copy_from.trim().is_empty() { None } else { entity.parse_id(&self.copy_from) };
                                    let db = db.clone();
                                    let sql = format!("SELECT 1 AS x FROM {} WHERE {} = ? LIMIT 1", crate::sql::ident(entity.table), crate::sql::ident(entity.key_col));
                                    self.exists.start(&ctx, move || {
                                        let found = !db.select(entity.db, &sql, std::slice::from_ref(&id))?.rows.is_empty();
                                        Ok((id, copy, found))
                                    });
                                }
                            }
                        }
                    });
                });
                ui.add_space(6.0);
            }

            if let Some(recent) = prefs.recent.get(entity.route).filter(|r| !r.is_empty()) {
                card(ui, |ui| {
                    ui.label(RichText::new("Récents").strong());
                    ui.horizontal_wrapped(|ui| {
                        for text in recent.clone() {
                            let Some(id) = entity.parse_id(&text) else { continue };
                            let name = entity.reference.and_then(|k| env.refs.name(k, &id)).flatten().unwrap_or_default();
                            if ui.button(format!("{text}  {name}")).clicked() {
                                open = Some(Open { id, is_new: false, copy_from: None });
                            }
                        }
                    });
                });
                ui.add_space(6.0);
            }

            card(ui, |ui| {
                ui.label(RichText::new("Rechercher").strong());
                let mut changed = false;
                ui.horizontal_wrapped(|ui| {
                    for (i, filter) in entity.search.filters.iter().enumerate() {
                        ui.vertical(|ui| {
                            ui.label(muted(filter.label));
                            match filter.values {
                                Some(values) => {
                                    let current = self.filters[i].clone();
                                    let label = values.iter().find(|(k, _)| k.to_string() == current).map(|(_, l)| l.to_string()).unwrap_or_else(|| "Tous".into());
                                    egui::ComboBox::from_id_salt(("filter", entity.route, filter.name)).width(180.0).selected_text(label).show_ui(ui, |ui| {
                                        if ui.add(egui::Button::selectable(current.is_empty(), "Tous")).clicked() {
                                            self.filters[i].clear();
                                            changed = true;
                                        }
                                        for (k, l) in values {
                                            if ui.add(egui::Button::selectable(current == k.to_string(), *l)).clicked() {
                                                self.filters[i] = k.to_string();
                                                changed = true;
                                            }
                                        }
                                    });
                                }
                                None => {
                                    changed |= ui.add(egui::TextEdit::singleline(&mut self.filters[i]).desired_width(180.0)).changed();
                                }
                            }
                        });
                    }
                });
                if changed {
                    self.page = 0;
                    self.stale = true;
                    prefs.filters.insert(entity.route.to_string(), self.filters.clone());
                }
                ui.add_space(4.0);
                if let Some(e) = &self.error {
                    ui.label(error(e));
                }
                let Some(set) = &self.results else {
                    ui.spinner();
                    return;
                };
                if set.rows.is_empty() {
                    ui.label(muted("Aucun résultat."));
                    return;
                }
                let key_index = set.columns.iter().position(|c| c == entity.key_col);
                egui::ScrollArea::horizontal().id_salt("results-h").show(ui, |ui| {
                    egui::Grid::new(("results", self.entity)).striped(true).show(ui, |ui| {
                        for c in &entity.search.columns {
                            ui.label(RichText::new(c.label).strong());
                        }
                        ui.end_row();
                        for row in set.rows.iter().take(PAGE) {
                            let mut clicked = false;
                            for (c, v) in entity.search.columns.iter().zip(row) {
                                let text = match (c.values, v.as_i64()) {
                                    (Some(values), Some(n)) => values.iter().find(|(k, _)| *k == n).map(|(_, l)| l.to_string()).unwrap_or_else(|| display(v)),
                                    _ => display(v),
                                };
                                let text: String = text.chars().take(80).collect();
                                let rich = if c.name == entity.key_col { RichText::new(text).color(ACCENT_LIGHT) } else { RichText::new(text) };
                                clicked |= ui.add(egui::Button::selectable(false, rich)).clicked();
                            }
                            if clicked {
                                if let Some(id) = key_index.and_then(|k| row.get(k)).cloned() {
                                    open = Some(Open { id, is_new: false, copy_from: None });
                                }
                            }
                            ui.end_row();
                        }
                    });
                });
                ui.horizontal(|ui| {
                    if ui.add_enabled(self.page > 0, egui::Button::new("‹ Précédent")).clicked() {
                        self.page -= 1;
                        self.stale = true;
                    }
                    ui.label(muted(format!("Page {}", self.page + 1)));
                    if ui.add_enabled(set.rows.len() > PAGE, egui::Button::new("Suivant ›")).clicked() {
                        self.page += 1;
                        self.stale = true;
                    }
                });
            });
        });
        open
    }
}
