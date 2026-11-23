//! Écran d'édition d'une entrée : en-tête, onglets, formulaire ou tableau de lignes, et panneau SQL
//! (changements ou entrée complète, à copier, exécuter ou ajouter à la couche de contenu).

use egui::{Color32, RichText, Ui};
use serde_json::Value;
use std::sync::Arc;

use super::common::{card, error, muted, primary, sql_job, Toasts, ACCENT, ACCENT_LIGHT, ERR, OK, WARN};
use super::widgets::{field_label, form_grid, value_input, widget, Env, BOX};
use crate::db::{ColumnInfo, Db, Schema};
use crate::defs::ENTITIES;
use crate::editor::{self, RowStatus, TabData};
use crate::model::{f, Entity, Field, Kind, TabKind};
use crate::task::Slot;

/// Demande de l'éditeur à l'application.
pub enum EditorRequest {
    /// Ajouter des requêtes à la couche de contenu.
    Content { title: String, sql: String, full: bool },
}

pub struct EditorView {
    pub entity: usize,
    pub id: Value,
    pub is_new: bool,
    pub copy_from: Option<Value>,
    tabs: Option<Vec<TabData>>,
    load: Slot<Vec<TabData>>,
    error: Option<String>,
    active: usize,
    full: bool,
    exec: Slot<()>,
}

impl EditorView {
    pub fn open(ctx: &egui::Context, db: &Arc<Db>, entity: usize, id: Value, is_new: bool, copy_from: Option<Value>) -> Self {
        let mut view = EditorView { entity, id, is_new, copy_from, tabs: None, load: Slot::default(), error: None, active: 0, full: false, exec: Slot::default() };
        view.reload(ctx, db);
        view
    }

    fn entity(&self) -> &'static Entity {
        &ENTITIES[self.entity]
    }

    pub fn reload(&mut self, ctx: &egui::Context, db: &Arc<Db>) {
        let (db, entity, id, is_new, copy) = (db.clone(), self.entity, self.id.clone(), self.is_new, self.copy_from.clone());
        self.load.start(ctx, move || editor::load(&db, &ENTITIES[entity], &id, is_new, copy.as_ref()));
    }

    pub fn select_tab(&mut self, id: &str) {
        if let Some(i) = self.entity().tabs.iter().position(|t| t.id == id) {
            self.active = i;
        }
    }

    pub fn title(&self) -> String {
        format!("{} {}", self.entity().singular, editor::display(&self.id))
    }

    pub fn dirty(&self) -> bool {
        let Some(tabs) = &self.tabs else { return false };
        self.entity().tabs.iter().zip(tabs).any(|(t, d)| !editor::diff_sql(t, d, &self.id).is_empty())
    }

    fn tab_dirty(&self, i: usize) -> bool {
        match &self.tabs {
            Some(tabs) => !editor::diff_sql(&self.entity().tabs[i], &tabs[i], &self.id).is_empty(),
            None => false,
        }
    }

    /// Exécute les changements des onglets donnés, une transaction par base.
    fn execute(&mut self, ctx: &egui::Context, db: &Arc<Db>, tabs: Vec<usize>) {
        let Some(data) = &self.tabs else { return };
        let entity = self.entity();
        let mut parts: Vec<(&'static str, String)> = Vec::new();
        for i in tabs {
            let sql = editor::diff_sql(&entity.tabs[i], &data[i], &self.id);
            if sql.is_empty() {
                continue;
            }
            let alias = entity.tab_db(&entity.tabs[i]);
            match parts.iter_mut().find(|(a, _)| *a == alias) {
                Some((_, s)) => {
                    s.push('\n');
                    s.push_str(&sql);
                }
                None => parts.push((alias, sql)),
            }
        }
        if parts.is_empty() {
            return;
        }
        let db = db.clone();
        self.exec.start(ctx, move || {
            for (alias, sql) in parts {
                db.script(alias, &sql, true)?;
            }
            Ok(())
        });
    }

    pub fn show(&mut self, ui: &mut Ui, env: &mut Env, db: &Arc<Db>, toasts: &mut Toasts) -> Option<EditorRequest> {
        let ctx = ui.ctx().clone();
        if let Some(r) = self.load.poll() {
            match r {
                Ok(tabs) => {
                    self.tabs = Some(tabs);
                    self.error = None;
                }
                Err(e) => self.error = Some(format!("{e:#}")),
            }
        }
        if let Some(r) = self.exec.poll() {
            match r {
                Ok(()) => {
                    toasts.ok("Requête exécutée. Redémarrer le serveur ArchCore pour qu'il relise le monde.");
                    if let Some(k) = self.entity().reference {
                        env.refs.forget(k, &self.id);
                    }
                    self.is_new = false;
                    self.copy_from = None;
                    self.reload(&ctx, db);
                }
                Err(e) => toasts.error(format!("Échec, rien n'a été écrit : {e:#}")),
            }
        }

        let entity = self.entity();
        let mut request = None;

        // En-tête : fil d'Ariane, titre, actions globales.
        ui.horizontal(|ui| {
            if ui.link(muted(entity.label)).clicked() {
                *env.nav = Some(super::widgets::Nav { route: entity.route, id: Value::Null });
            }
        });
        ui.horizontal(|ui| {
            ui.heading(RichText::new(self.title()).strong());
            if let Some(k) = entity.reference {
                if !self.is_new {
                    if let Some(Some(n)) = env.refs.name(k, &self.id) {
                        ui.heading(RichText::new(n).color(ACCENT_LIGHT));
                    }
                }
            }
            if self.is_new {
                let badge = match &self.copy_from {
                    Some(c) => format!("nouvelle, copie de {}", editor::display(c)),
                    None => "nouvelle".into(),
                };
                ui.label(RichText::new(badge).color(OK));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let dirty_tabs: Vec<usize> = (0..entity.tabs.len()).filter(|i| self.tab_dirty(*i)).collect();
                let blocked = self.tabs.as_ref().is_some_and(|data| dirty_tabs.iter().any(|i| !editor::errors(&entity.tabs[*i], &data[*i]).is_empty()));
                let busy = self.exec.busy();
                if ui.add_enabled(!dirty_tabs.is_empty() && !blocked && !busy, primary("Tout enregistrer")).clicked() {
                    self.execute(&ctx, db, dirty_tabs.clone());
                }
                let world_sql: Vec<String> = match &self.tabs {
                    Some(data) => dirty_tabs
                        .iter()
                        .filter(|i| entity.tab_db(&entity.tabs[**i]) == "world")
                        .map(|i| format!("-- {}\n{}", entity.tabs[*i].label, editor::diff_sql(&entity.tabs[*i], &data[*i], &self.id)))
                        .collect(),
                    None => Vec::new(),
                };
                if ui.add_enabled(!world_sql.is_empty() && !blocked, egui::Button::new("Tout ajouter au contenu")).clicked() {
                    request = Some(EditorRequest::Content { title: self.title(), sql: world_sql.join("\n\n"), full: false });
                }
                if ui.button("Recharger").clicked() {
                    if self.dirty() {
                        env.modals.confirm = Some(super::common::Confirm {
                            title: "Recharger".into(),
                            message: "Abandonner les modifications non enregistrées ?".into(),
                            ok: "Abandonner".into(),
                            action: super::common::ConfirmAction::Reload,
                        });
                    } else {
                        self.reload(&ctx, db);
                    }
                }
            });
        });
        if !entity.help.is_empty() {
            ui.label(muted(entity.help));
        }
        if let Some(e) = &self.error {
            ui.label(error(e));
            return None;
        }
        if self.tabs.is_none() {
            ui.spinner();
            return None;
        }

        // Onglets.
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for (i, tab) in entity.tabs.iter().enumerate() {
                let count = match &self.tabs.as_ref().unwrap()[i] {
                    TabData::Multi { rows, .. } => format!(" ({})", rows.iter().filter(|r| !r.deleted).count()),
                    _ => String::new(),
                };
                let dot = if self.tab_dirty(i) { " *" } else { "" };
                let mut text = RichText::new(format!("{}{count}{dot}", tab.label));
                if i == self.active {
                    text = text.strong().color(ACCENT_LIGHT);
                }
                let resp = ui.add(egui::Button::selectable(i == self.active, text));
                if resp.clicked() {
                    self.active = i;
                }
            }
        });
        ui.separator();

        let tab = &entity.tabs[self.active];
        let alias = entity.tab_db(tab);
        let table_missing = tab.table().is_some_and(|t| !db.schema.has_table(alias, t));

        // Panneau SQL en bas (pas pour les listes en lecture seule).
        if !matches!(tab.kind, TabKind::List(_)) && !table_missing {
            egui::Panel::bottom("editor-sql").resizable(true).default_size(210.0).min_size(120.0).show(ui, |ui| {
                if let Some(r) = self.query_output(ui, db) {
                    request = Some(r);
                }
            });
        }

        egui::CentralPanel::no_frame().show(ui, |ui| {
            egui::ScrollArea::vertical().id_salt(("tab", tab.id)).auto_shrink(false).show(ui, |ui| {
                if !tab.help.is_empty() {
                    ui.label(muted(tab.help));
                    ui.add_space(4.0);
                }
                if table_missing {
                    ui.label(error(format!(
                        "La table {} n'existe pas dans cette base : elle vient d'une version plus récente d'ArchCore (lancer `archcore db-init`).",
                        tab.table().unwrap_or_default()
                    )));
                    return;
                }
                let id = self.id.clone();
                let data = &mut self.tabs.as_mut().unwrap()[self.active];
                let base = egui::Id::unique(("tab", self.entity, self.active));
                match (&tab.kind, data) {
                    (TabKind::Single(s), TabData::Single { after, .. }) => single_tab(ui, env, &db.schema, alias, s, after, &id, base),
                    (TabKind::Multi(m), TabData::Multi { rows, columns }) => multi_tab(ui, env, m, rows, columns, &id, base),
                    (TabKind::List(l), TabData::List { rows }) => list_tab(ui, env, l, rows),
                    _ => {}
                }
            });
        });
        request
    }

    fn query_output(&mut self, ui: &mut Ui, db: &Arc<Db>) -> Option<EditorRequest> {
        let entity = self.entity();
        let tab = &entity.tabs[self.active];
        let data = &self.tabs.as_ref().unwrap()[self.active];
        let sql = if self.full { editor::full_sql(tab, data, &self.id) } else { editor::diff_sql(tab, data, &self.id) };
        let errors = editor::errors(tab, data);
        let world = entity.tab_db(tab) == "world";
        let mut request = None;
        let mut run = false;
        ui.horizontal(|ui| {
            if ui.add(egui::Button::selectable(!self.full, "Changements")).clicked() {
                self.full = false;
            }
            if ui.add(egui::Button::selectable(self.full, "Entrée complète")).clicked() {
                self.full = true;
            }
            if !errors.is_empty() {
                ui.label(RichText::new(format!("À corriger : {}", errors.join(" ; "))).color(ERR));
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let blocked = sql.is_empty() || !errors.is_empty() || self.exec.busy();
                if ui.add_enabled(!blocked && !self.full, primary("Exécuter")).on_hover_text("Les changements, dans une transaction").clicked() {
                    run = true;
                }
                let content = ui.add_enabled(!blocked && world, egui::Button::new("Ajouter au contenu"));
                let content = if world { content } else { content.on_disabled_hover_text("La couche de contenu ne vise que la base monde") };
                if content.clicked() {
                    request = Some(EditorRequest::Content { title: format!("{} — {}", self.title(), tab.label), sql: sql.clone(), full: self.full });
                }
                if ui.add_enabled(!sql.is_empty(), egui::Button::new("Copier")).clicked() {
                    ui.ctx().copy_text(sql.clone());
                }
            });
        });
        egui::ScrollArea::vertical().id_salt("sql-out").auto_shrink(false).show(ui, |ui| {
            egui::Frame::new().fill(Color32::from_rgb(14, 14, 16)).corner_radius(6.0).inner_margin(10.0).show(ui, |ui| {
                ui.set_width(ui.available_width());
                if sql.is_empty() {
                    ui.label(muted(if self.full { "-- Entrée vide." } else { "-- Aucun changement." }).monospace());
                } else {
                    ui.add(egui::Label::new(sql_job(&sql, ui.available_width())).selectable(true));
                }
            });
        });
        if run {
            let i = self.active;
            self.execute(ui.ctx(), db, vec![i]);
        }
        request
    }
}

/// Champ générique pour une colonne que les définitions ne décrivent pas (ajoutée par une migration récente).
fn column_field(c: &ColumnInfo) -> Field {
    let name: &'static str = crate::defs::leak(c.name.clone());
    let mut field = f(name, name);
    field.kind = if c.is_integer() {
        Kind::Int
    } else if c.is_real() {
        Kind::Float
    } else {
        Kind::Text
    };
    field.nullable = c.nullable;
    field.readonly = c.is_binary();
    field
}

fn single_tab(ui: &mut Ui, env: &mut Env, schema: &Schema, alias: &str, s: &crate::model::SingleTab, after: &mut Option<crate::sql::Row>, id: &Value, base: egui::Id) {
    let Some(row) = after else {
        card(ui, |ui| {
            ui.label(format!("Aucune ligne dans {} pour cette entrée.", s.table));
            if ui.add(primary("Créer la ligne")).clicked() {
                let mut r = editor::attach(&(s.key)(id));
                r.extend((s.defaults)(id));
                *after = Some(r);
            }
        });
        return;
    };
    let columns = schema.columns(alias, s.table).unwrap_or(&[]);
    let comment = |name: &str| columns.iter().find(|c| c.name == name).map(|c| c.comment.as_str()).unwrap_or("");
    let key_cols: Vec<String> = editor::attach(&(s.key)(id)).keys().cloned().collect();
    // Champs déclarés présents dans la table (une colonne d'une version plus récente peut manquer).
    let fields: Vec<&Field> = s.fields.iter().filter(|f| columns.is_empty() || columns.iter().any(|c| c.name == f.name)).collect();

    let mut groups: Vec<(Option<&'static str>, Vec<&Field>)> = Vec::new();
    for field in fields.iter().filter(|f| !f.kind.is_big()) {
        match groups.iter_mut().find(|(g, _)| *g == field.group) {
            Some((_, list)) => list.push(field),
            None => groups.push((field.group, vec![field])),
        }
    }
    for (group, list) in groups {
        let mut draw = |ui: &mut Ui| {
            form_grid(ui, base.with(("grid", group)), &list, |ui, field| {
                let field = &Field { readonly: field.readonly || key_cols.iter().any(|k| k == field.name), ..(*field).clone() };
                let mut v = row.get(field.name).cloned().unwrap_or(Value::Null);
                field_label(ui, field, comment(field.name));
                if value_input(ui, env, field, &mut v, base.with(field.name), BOX - 10.0, false) {
                    row.insert(field.name.to_string(), v);
                }
            });
        };
        match group {
            Some(g) => {
                egui::CollapsingHeader::new(RichText::new(g).strong().color(ACCENT_LIGHT)).id_salt(base.with(g)).default_open(true).show(ui, draw);
            }
            None => draw(ui),
        }
    }
    for field in fields.iter().filter(|f| f.kind.is_big()) {
        ui.add_space(8.0);
        card(ui, |ui| {
            field_label(ui, field, comment(field.name));
            let mut v = row.get(field.name).cloned().unwrap_or(Value::Null);
            if widget(ui, env, field, &mut v, base.with(field.name)) {
                row.insert(field.name.to_string(), v);
            }
        });
    }

    // Colonnes de la table que les définitions ne décrivent pas : éditables, expliquées par leur commentaire.
    let others: Vec<&ColumnInfo> = columns.iter().filter(|c| !s.fields.iter().any(|f| f.name == c.name) && row.contains_key(&c.name)).collect();
    if !others.is_empty() {
        ui.add_space(8.0);
        egui::CollapsingHeader::new(RichText::new(format!("Colonnes non décrites ({})", others.len())).color(WARN))
            .id_salt(base.with("other-columns"))
            .default_open(false)
            .show(ui, |ui| {
                ui.label(muted("Colonnes ajoutées par une version d'ArchCore plus récente qu'ArchForge : leur explication vient du schéma."));
                let editable: Vec<&ColumnInfo> = others.into_iter().filter(|c| row.get(&c.name).and_then(crate::db::binary_len).is_none()).collect();
                form_grid(ui, base.with("other-grid"), &editable, |ui, c| {
                    let field = column_field(c);
                    let mut v = row.get(&c.name).cloned().unwrap_or(Value::Null);
                    field_label(ui, &field, &c.comment);
                    if value_input(ui, env, &field, &mut v, base.with(("col", c.name.as_str())), BOX - 10.0, false) {
                        row.insert(c.name.clone(), v);
                    }
                });
            });
    }
}

fn column_width(field: &Field) -> f32 {
    match field.kind {
        Kind::Ref(_) => 220.0,
        Kind::Enum(_) | Kind::TextEnum(_) => 170.0,
        Kind::Bool => 40.0,
        Kind::Int | Kind::Float | Kind::Flags(..) => 80.0,
        _ => 140.0,
    }
}

fn multi_tab(ui: &mut Ui, env: &mut Env, m: &crate::model::MultiTab, rows: &mut Vec<editor::RowState>, columns: &[String], id: &Value, base: egui::Id) {
    let fields: Vec<&Field> = m.columns.iter().filter(|f| columns.is_empty() || columns.iter().any(|c| c == f.name)).collect();
    ui.horizontal(|ui| {
        if ui.add(primary("+ Ajouter une ligne")).clicked() {
            let after = editor::add_row(m, id, rows);
            rows.push(editor::RowState { before: None, after, deleted: false });
        }
        ui.label(muted(format!("{} ligne(s) — clé : {}", rows.iter().filter(|r| !r.deleted).count(), m.row_key.join(", "))));
    });
    let mut action: Option<(usize, &str)> = None;
    egui::ScrollArea::horizontal().id_salt(base.with("h")).show(ui, |ui| {
        egui::Grid::new(base.with("grid")).striped(true).min_col_width(20.0).show(ui, |ui| {
            ui.label("");
            for field in &fields {
                // En-tête compact : le libellé, coloré si le serveur lit la colonne ; le reste au survol.
                let color = if field.server { ACCENT_LIGHT } else { Color32::from_rgb(220, 216, 210) };
                let hover = format!("{}{}{}", field.name, if field.server { " — lu par le serveur" } else { "" }, if field.help.is_empty() { String::new() } else { format!("
{}", field.help) });
                let width = if field.kind.is_json() { 140.0 } else { column_width(field) };
                ui.allocate_ui_with_layout(egui::vec2(width, 18.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_width(width);
                    ui.label(RichText::new(field.label).strong().color(color)).on_hover_text(hover);
                });
            }
            ui.label("");
            ui.end_row();
            for (i, r) in rows.iter_mut().enumerate() {
                let status = r.status();
                let (mark, color) = match status {
                    RowStatus::Added => ("+", OK),
                    RowStatus::Changed => ("*", ACCENT),
                    RowStatus::Deleted => ("×", ERR),
                    RowStatus::Same => ("", Color32::GRAY),
                };
                ui.label(RichText::new(format!("{} {mark}", i + 1)).color(color));
                let deleted = r.deleted;
                for field in &fields {
                    let locked = r.before.is_some() && m.row_key.contains(&field.name);
                    let field = &Field { readonly: field.readonly || deleted || locked, ..(*field).clone() };
                    let cell = base.with((i, field.name));
                    let mut v = r.after.get(field.name).cloned().unwrap_or(Value::Null);
                    if let Some(nv) = env.modals.take_result(cell.with("json")) {
                        r.after.insert(field.name.to_string(), nv);
                        continue_cell(ui, &v);
                        continue;
                    }
                    if field.kind.is_json() {
                        ui.horizontal(|ui| {
                            if ui.add_enabled(!field.readonly, egui::Button::new("Modifier").small()).clicked() {
                                env.modals.json = Some(super::common::JsonDialog {
                                    target: cell.with("json"),
                                    title: format!("{} — ligne {}", field.label, i + 1),
                                    field: field.clone(),
                                    value: v.clone(),
                                });
                            }
                            let preview: String = v.to_string().chars().take(40).collect();
                            ui.label(muted(preview).monospace().small());
                        });
                        continue;
                    }
                    if value_input(ui, env, field, &mut v, cell, column_width(field), true) {
                        r.after.insert(field.name.to_string(), v);
                    }
                }
                ui.horizontal(|ui| {
                    if deleted {
                        if ui.small_button("‹").on_hover_text("Restaurer").clicked() {
                            action = Some((i, "restore"));
                        }
                    } else {
                        if ui.small_button("+").on_hover_text("Dupliquer").clicked() {
                            action = Some((i, "dup"));
                        }
                        if ui.small_button("×").on_hover_text("Supprimer").clicked() {
                            action = Some((i, "del"));
                        }
                    }
                });
                ui.end_row();
            }
        });
    });
    match action {
        Some((i, "restore")) => rows[i].deleted = false,
        Some((i, "dup")) => {
            let after = editor::new_row_from(m, id, rows[i].after.clone());
            rows.insert(i + 1, editor::RowState { before: None, after, deleted: false });
        }
        Some((i, "del")) => {
            if rows[i].before.is_some() {
                rows[i].deleted = true;
            } else {
                rows.remove(i);
            }
        }
        _ => {}
    }
}

/// Cellule déjà mise à jour par une fenêtre : affichage minimal pour cette image.
fn continue_cell(ui: &mut Ui, v: &Value) {
    ui.label(muted(v.to_string().chars().take(40).collect::<String>()).monospace().small());
}

fn list_tab(ui: &mut Ui, env: &mut Env, l: &crate::model::ListTab, rows: &[crate::sql::Row]) {
    if rows.is_empty() {
        ui.label(muted(l.empty));
        return;
    }
    egui::Grid::new("list-tab").striped(true).show(ui, |ui| {
        for (_, label, _) in &l.columns {
            ui.label(RichText::new(*label).strong());
        }
        ui.end_row();
        for row in rows {
            for (col, _, reference) in &l.columns {
                let v = row.get(*col).cloned().unwrap_or(Value::Null);
                match reference {
                    Some(r) => {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(editor::display(&v)).monospace());
                            if let Some(Some(n)) = env.refs.name(*r, &v) {
                                ui.label(RichText::new(n).color(ACCENT_LIGHT));
                            }
                            if let Some(route) = crate::model::ref_def(*r).route {
                                if ui.small_button("»").clicked() {
                                    *env.nav = Some(super::widgets::Nav { route, id: v.clone() });
                                }
                            }
                        });
                    }
                    None => {
                        ui.label(editor::display(&v));
                    }
                }
            }
            ui.end_row();
        }
    });
}

#[cfg(test)]
mod tests {
    //! Tests d'interface sans fenêtre (egui_kittest) : frappes et clics sur les vrais widgets, puis SQL produit.
    use super::*;
    use crate::model::row;
    use crate::sql::Row;
    use crate::ui::common::{Modals, RefCache};
    use egui::accesskit::Role;
    use egui_kittest::{kittest::Queryable, Harness};
    use serde_json::json;

    #[derive(Default)]
    struct State {
        single: Option<Row>,
        rows: Vec<editor::RowState>,
        refs: RefCache,
        modals: Modals,
        nav: Option<super::super::widgets::Nav>,
    }

    fn tab(route: &str, id: &str) -> &'static crate::model::Tab {
        let entity = &ENTITIES[crate::defs::by_route(route).unwrap()];
        entity.tabs.iter().find(|t| t.id == id).unwrap()
    }

    #[test]
    fn saisie_dans_une_cle_json_produit_un_json_set() {
        let tab = tab("creature", "data");
        let TabKind::Single(s) = &tab.kind else { panic!() };
        let id = json!(459);
        let before = row(&[("tid", json!(459)), ("name", json!("Young Hydra")), ("data", json!({"LVL": "13", "MAX_HP": "366"}))]);
        let state = State { single: Some(before.clone()), ..Default::default() };
        let schema = Schema::default();
        let mut harness = Harness::builder().with_accessibility_check(false).build_ui_state(
            |ui, st: &mut State| {
                let mut env = Env { refs: &mut st.refs, modals: &mut st.modals, nav: &mut st.nav };
                egui::ScrollArea::vertical().show(ui, |ui| single_tab(ui, &mut env, &schema, "world", s, &mut st.single, &id, egui::Id::unique("t")));
            },
            state,
        );
        harness.run();
        Queryable::get(&harness, egui_kittest::kittest::by().role(Role::TextInput).value("366")).focus();
        harness.run();
        harness.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
        Queryable::get(&harness, egui_kittest::kittest::by().role(Role::TextInput).value("366")).type_text("400");
        harness.run();
        let data = TabData::Single { before: Some(before), after: harness.state().single.clone() };
        assert_eq!(
            editor::diff_sql(tab, &data, &id),
            "UPDATE `character_data_table` SET `data` = JSON_SET(`data`, '$.\"MAX_HP\"', '400') WHERE `tid` = 459;"
        );
    }

    #[test]
    fn ajout_d_une_ligne_de_butin_produit_un_insert() {
        let tab = tab("creature", "loot");
        let TabKind::Multi(m) = &tab.kind else { panic!() };
        let id = json!(459);
        let columns: Vec<String> = ["id", "stable_key", "monster_tid", "item_tid", "min_count", "max_count", "rate"].map(String::from).to_vec();
        let mut harness = Harness::builder().with_accessibility_check(false).build_ui_state(
            |ui, st: &mut State| {
                let mut env = Env { refs: &mut st.refs, modals: &mut st.modals, nav: &mut st.nav };
                multi_tab(ui, &mut env, m, &mut st.rows, &columns, &id, egui::Id::unique("m"));
            },
            State::default(),
        );
        harness.run();
        harness.get_by_label("+ Ajouter une ligne").click();
        harness.run();
        assert_eq!(harness.state().rows.len(), 1);
        // L'objet se saisit dans le premier champ numérique vide de la ligne (TID 0 au départ).
        let inputs = harness.get_all_by_role(Role::TextInput).count();
        assert!(inputs >= 5, "champs de la ligne : {inputs}");
        let data = TabData::Multi { rows: harness.state().rows.clone(), columns: Vec::new() };
        let sql = editor::diff_sql(tab, &data, &id);
        assert!(sql.starts_with("INSERT INTO `monster_drop` (`id`, `stable_key`, `monster_tid`"), "{sql}");
        assert!(sql.contains("SELECT COALESCE(MAX(`id`), 0) + 1, '459:0#af"), "{sql}");
    }
}

#[cfg(test)]
mod glyphes {
    /// Les symboles des boutons doivent exister dans les polices embarquées d'egui : sans police emoji du
    /// système (Linux), un symbole absent s'afficherait en carré.
    #[test]
    fn symboles_des_boutons_dans_les_polices_embarquees() {
        let mut h = egui_kittest::Harness::new_ui(|ui| {
            for c in ['…', '»', '×', '+', '‹', '›', '*'] {
                assert!(ui.ctx().fonts_mut(|f| f.has_glyph(&egui::FontId::proportional(14.0), c)), "pas de glyphe pour {c}");
            }
        });
        h.run();
    }
}
