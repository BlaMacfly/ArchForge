//! Champs de l'éditeur : chaque champ se dessine selon sa sorte (`model::Kind`), avec son explication au
//! survol, le badge « serveur » et, pour une référence, le nom de l'entrée visée, un sélecteur et un lien.

use egui::{Align, Color32, Id, Layout, RichText, Ui};
use serde_json::{Map, Value};

use super::common::{id_text, muted, Modals, RefCache, ACCENT, ACCENT_LIGHT, ERR};
use crate::editor::{display, is_valid, parse_input};
use crate::model::{f, ref_def, Field, Kind, RefKind, Section};

/// Navigation demandée par un champ (lien vers l'entrée référencée).
pub struct Nav {
    pub route: &'static str,
    pub id: Value,
}

/// Ce dont un champ a besoin autour de lui.
pub struct Env<'a> {
    pub refs: &'a mut RefCache,
    pub modals: &'a mut Modals,
    pub nav: &'a mut Option<Nav>,
}

/// Largeur d'une case de champ dans les formulaires.
pub const BOX: f32 = 250.0;

pub fn field_label(ui: &mut Ui, field: &Field, comment: &str) {
    let help = if field.help.is_empty() { comment } else { field.help };
    let resp = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.label(RichText::new(field.label).strong());
            if field.server {
                ui.label(RichText::new("serveur").small().color(ACCENT_LIGHT));
            }
            if !help.is_empty() {
                ui.label(RichText::new("?").small().color(Color32::from_rgb(120, 116, 110)));
            }
            ui.label(RichText::new(field.name).small().monospace().color(Color32::from_rgb(110, 106, 100)));
        })
        .response;
    if !help.is_empty() {
        resp.on_hover_text(help);
    }
}

/// Formulaire en grille : autant de colonnes de largeur `BOX` que la place le permet, cases alignées en haut.
pub fn form_grid<T>(ui: &mut Ui, id: Id, items: &[T], mut cell: impl FnMut(&mut Ui, &T)) {
    let cols = ((ui.available_width() + 12.0) / (BOX + 12.0)).floor().max(1.0) as usize;
    egui::Grid::new(id).num_columns(cols).spacing([12.0, 10.0]).min_col_width(BOX).max_col_width(BOX).show(ui, |ui| {
        for (i, item) in items.iter().enumerate() {
            ui.allocate_ui_with_layout(egui::vec2(BOX, 0.0), Layout::top_down(Align::LEFT), |ui| {
                ui.set_width(BOX);
                cell(ui, item);
            });
            if (i + 1) % cols == 0 {
                ui.end_row();
            }
        }
    });
}

fn as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64(),
        Value::String(s) => s.trim().parse().ok(),
        Value::Bool(b) => Some(*b as i64),
        _ => None,
    }
}

fn invalid_frame(ui: &Ui, rect: egui::Rect) {
    ui.painter().rect_stroke(rect, 3.0, egui::Stroke::new(1.5, ERR), egui::StrokeKind::Outside);
}

fn text_input(ui: &mut Ui, field: &Field, value: &mut Value, width: f32) -> bool {
    let mut text = display(value);
    let mut edit = egui::TextEdit::singleline(&mut text).desired_width(width);
    if value.is_null() {
        edit = edit.hint_text("NULL");
    }
    if field.kind.is_numeric() || matches!(field.kind, Kind::Float) {
        edit = edit.font(egui::TextStyle::Monospace);
    }
    let resp = ui.add_enabled(!field.readonly, edit);
    let changed = resp.changed();
    if changed {
        *value = parse_input(&text, field);
    }
    if !is_valid(value, field) {
        invalid_frame(ui, resp.rect);
    }
    changed
}

fn ref_name(ui: &mut Ui, env: &mut Env, kind: RefKind, value: &Value) {
    let Some(text) = id_text(value) else { return };
    if text == "0" {
        return;
    }
    match env.refs.name(kind, value) {
        Some(Some(n)) => {
            ui.label(RichText::new(n).color(ACCENT_LIGHT));
        }
        Some(None) => {
            ui.label(RichText::new("introuvable").color(ERR));
        }
        None => {}
    }
    if let Some(route) = ref_def(kind).route {
        if ui.small_button("»").on_hover_text("Ouvrir").clicked() {
            *env.nav = Some(Nav { route, id: value.clone() });
        }
    }
}

/// Champ simple (nombre, texte, liste de valeurs, drapeaux, référence). Renvoie vrai s'il a changé.
pub fn value_input(ui: &mut Ui, env: &mut Env, field: &Field, value: &mut Value, id: Id, width: f32, compact: bool) -> bool {
    let mut changed = false;
    if let Some(v) = env.modals.take_result(id) {
        *value = v;
        changed = true;
    }
    match &field.kind {
        Kind::Bool => {
            let mut on = as_i64(value).unwrap_or(0) != 0;
            if ui.add_enabled(!field.readonly, egui::Checkbox::without_text(&mut on)).changed() {
                *value = Value::from(on as i64);
                changed = true;
            }
        }
        Kind::Enum(values) => {
            let current = as_i64(value);
            let label = values
                .iter()
                .find(|(k, _)| Some(*k) == current)
                .map(|(k, l)| if compact { l.to_string() } else { format!("{k} — {l}") })
                .unwrap_or_else(|| format!("{} (valeur inconnue)", display(value)));
            ui.add_enabled_ui(!field.readonly, |ui| {
                egui::ComboBox::from_id_salt(id).width(width).selected_text(label).show_ui(ui, |ui| {
                    for (k, l) in *values {
                        if ui.add(egui::Button::selectable(current == Some(*k), format!("{k} — {l}"))).clicked() {
                            *value = Value::from(*k);
                            changed = true;
                        }
                    }
                });
            });
        }
        Kind::TextEnum(values) => {
            let current = display(value);
            let label = values.iter().find(|(k, _)| *k == current).map(|(_, l)| l.to_string()).unwrap_or(current.clone());
            ui.add_enabled_ui(!field.readonly, |ui| {
                egui::ComboBox::from_id_salt(id).width(width).selected_text(label).show_ui(ui, |ui| {
                    for (k, l) in *values {
                        if ui.add(egui::Button::selectable(current == *k, *l)).clicked() {
                            *value = Value::from(*k);
                            changed = true;
                        }
                    }
                });
            });
        }
        Kind::Flags(bits, none) => {
            ui.horizontal(|ui| {
                changed |= text_input(ui, field, value, (width - 40.0).max(60.0));
                if ui.add_enabled(!field.readonly, egui::Button::new("bits").small()).on_hover_text("Choisir les bits").clicked() {
                    env.modals.flags = Some(super::common::FlagsDialog {
                        target: id,
                        title: field.label.to_string(),
                        help: field.help.to_string(),
                        bits,
                        value: as_i64(value).unwrap_or(0),
                    });
                }
            });
            if !compact {
                let n = as_i64(value).unwrap_or(0);
                let on: Vec<&str> = bits.iter().filter(|(b, _)| n & b != 0).map(|(_, l)| *l).collect();
                ui.label(muted(if on.is_empty() { none.to_string() } else { on.join(", ") }).small());
            }
        }
        Kind::Ref(kind) => {
            ui.horizontal(|ui| {
                changed |= text_input(ui, field, value, if compact { 70.0 } else { 90.0 });
                if ui.add_enabled(!field.readonly, egui::Button::new("…").small()).on_hover_text(format!("Choisir : {}", ref_def(*kind).label)).clicked() {
                    env.modals.open_picker(id, *kind);
                }
                ref_name(ui, env, *kind, value);
            });
        }
        _ => changed |= text_input(ui, field, value, width),
    }
    changed
}

/// Champ quelconque, y compris les grandes sortes (JSON, paires, listes).
pub fn widget(ui: &mut Ui, env: &mut Env, field: &Field, value: &mut Value, id: Id) -> bool {
    match &field.kind {
        Kind::Json(sections) => json_object(ui, env, sections, value, id),
        Kind::Pairs => pairs(ui, env, value, id),
        Kind::List { reference, text } => list(ui, env, *reference, *text, value, id),
        Kind::PairList { reference, left, right } => pair_list(ui, env, *reference, left, right, value, id),
        _ => value_input(ui, env, field, value, id, BOX - 10.0, false),
    }
}

fn temp_string(ui: &Ui, id: Id) -> String {
    ui.data(|d| d.get_temp::<String>(id)).unwrap_or_default()
}

fn set_temp_string(ui: &Ui, id: Id, s: String) {
    ui.data_mut(|d| d.insert_temp(id, s));
}

// ------------------------------------------------------------------ objet JSON

/// Colonne JSON objet (raw, data, spec…) : clés connues regroupées par section, puis toutes les autres.
pub fn json_object(ui: &mut Ui, env: &mut Env, sections: &[Section], value: &mut Value, id: Id) -> bool {
    let original: Map<String, Value> = value.as_object().cloned().unwrap_or_default();
    let mut obj = original.clone();
    let mut changed = false;
    let known: Vec<&str> = sections.iter().flat_map(|s| s.keys.iter().map(|k| k.name)).collect();

    for section in sections {
        let present = section.keys.iter().filter(|k| obj.contains_key(k.name)).count();
        egui::CollapsingHeader::new(RichText::new(format!("{}  ({present}/{})", section.title, section.keys.len())).color(ACCENT_LIGHT))
            .id_salt(id.with(section.title))
            .default_open(present > 0)
            .show(ui, |ui| {
                form_grid(ui, id.with((section.title, "grid")), &section.keys, |ui, key| {
                        let kfield = Field { nullable: false, allow_empty: true, ..key.clone() };
                        let had = original.get(key.name).cloned();
                        let mut v = obj.get(key.name).cloned().unwrap_or(Value::String(String::new()));
                        {
                            field_label(ui, &kfield, "");
                            let kid = id.with(key.name);
                            if value_input(ui, env, &kfield, &mut v, kid, BOX - 10.0, false) {
                                changed = true;
                                // Les tables du client gardent tout en texte : la valeur y reste en texte.
                                let keep_text = had.as_ref().is_none_or(|h| h.is_string());
                                if keep_text {
                                    if let Value::Number(n) = &v {
                                        v = Value::String(n.to_string());
                                    }
                                }
                                if had.is_none() && v == Value::String(String::new()) {
                                    obj.remove(key.name);
                                } else {
                                    obj.insert(key.name.to_string(), v);
                                }
                            }
                            if had.is_none() {
                                ui.label(muted("absente").small());
                            }
                        }
                });
            });
    }

    let others: Vec<String> = obj.keys().filter(|k| !known.contains(&k.as_str())).cloned().collect();
    egui::CollapsingHeader::new(RichText::new(format!("{} ({})", if sections.is_empty() { "Clés" } else { "Autres clés" }, others.len())).color(ACCENT_LIGHT))
        .id_salt(id.with("others"))
        .default_open(sections.is_empty())
        .show(ui, |ui| {
            let fid = id.with("filter");
            let mut filter = temp_string(ui, fid);
            if ui.add(egui::TextEdit::singleline(&mut filter).hint_text("Filtrer les clés…").desired_width(260.0)).changed() {
                set_temp_string(ui, fid, filter.clone());
            }
            let fl = filter.to_lowercase();
            egui::ScrollArea::vertical().id_salt(id.with("others-scroll")).max_height(320.0).show(ui, |ui| {
                egui::Grid::new(id.with("others-grid")).striped(true).num_columns(3).show(ui, |ui| {
                    for k in &others {
                        let v = obj.get(k).cloned().unwrap_or(Value::Null);
                        if !fl.is_empty() && !k.to_lowercase().contains(&fl) && !display(&v).to_lowercase().contains(&fl) {
                            continue;
                        }
                        ui.label(RichText::new(k).monospace());
                        if v.is_object() || v.is_array() {
                            ui.label(muted(v.to_string()).monospace());
                        } else {
                            let mut text = display(&v);
                            if ui.add(egui::TextEdit::singleline(&mut text).desired_width(320.0)).changed() {
                                let nv = if v.is_number() { text.parse::<f64>().ok().and_then(|_| serde_json::from_str(&text).ok()).unwrap_or(Value::String(text)) } else { Value::String(text) };
                                obj.insert(k.clone(), nv);
                                changed = true;
                            }
                        }
                        if ui.small_button("×").on_hover_text("Retirer la clé").clicked() {
                            obj.remove(k);
                            changed = true;
                        }
                        ui.end_row();
                    }
                });
            });
            ui.horizontal(|ui| {
                let (kid, vid) = (id.with("new-key"), id.with("new-value"));
                let (mut nk, mut nv) = (temp_string(ui, kid), temp_string(ui, vid));
                ui.add(egui::TextEdit::singleline(&mut nk).hint_text("Nouvelle clé").desired_width(180.0));
                ui.add(egui::TextEdit::singleline(&mut nv).hint_text("Valeur").desired_width(180.0));
                if ui.button("Ajouter la clé").clicked() && !nk.trim().is_empty() {
                    obj.insert(nk.trim().to_string(), Value::String(nv.clone()));
                    changed = true;
                    nk.clear();
                    nv.clear();
                }
                set_temp_string(ui, kid, nk);
                set_temp_string(ui, vid, nv);
            });
        });
    if changed {
        *value = Value::Object(obj);
    }
    changed
}

// ------------------------------------------------------------------ paires de npc.ini

fn function_label(n: i64) -> String {
    crate::defs::npc_function(n).map(str::to_string).unwrap_or_else(|| format!("fonction {n}"))
}

/// Paires [clé, valeur] de npc.ini : chaque « Function » ouvre un événement du PNJ.
pub fn pairs(ui: &mut Ui, env: &mut Env, value: &mut Value, id: Id) -> bool {
    let mut list: Vec<(String, String)> = value
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|p| p.as_array().map(|p| (p.first().map(display).unwrap_or_default(), p.get(1).map(display).unwrap_or_default())))
                .collect()
        })
        .unwrap_or_default();
    let mut changed = false;
    let mut remove = None;
    let mut function = None;
    egui::Grid::new(id.with("pairs")).striped(true).num_columns(3).show(ui, |ui| {
        for (i, (k, v)) in list.iter_mut().enumerate() {
            if k == "Function" {
                function = v.trim().parse::<i64>().ok();
                ui.label(RichText::new(format!("Function — {}", function_label(function.unwrap_or(-1)))).strong().color(ACCENT));
            } else {
                ui.label(RichText::new(k.as_str()).monospace());
            }
            let reference = match (k.as_str(), function) {
                ("Template", Some(9)) => Some(RefKind::Shop),
                ("QuestGroupID", _) => Some(RefKind::QuestGroup),
                ("TID", _) => Some(RefKind::Creature),
                _ => None,
            };
            let field = match reference {
                Some(r) => f("v", "").reference(r),
                None => f("v", ""),
            };
            let mut val = Value::String(v.clone());
            if value_input(ui, env, &field, &mut val, id.with(("pair", i)), 260.0, true) {
                *v = display(&val);
                changed = true;
            }
            if ui.small_button("×").on_hover_text("Retirer la paire").clicked() {
                remove = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        list.remove(i);
        changed = true;
    }
    ui.horizontal(|ui| {
        let (kid, vid) = (id.with("new-key"), id.with("new-value"));
        let (mut nk, mut nv) = (temp_string(ui, kid), temp_string(ui, vid));
        ui.add(egui::TextEdit::singleline(&mut nk).hint_text("Clé (Function, Template…)").desired_width(200.0));
        ui.add(egui::TextEdit::singleline(&mut nv).hint_text("Valeur").desired_width(160.0));
        if ui.button("Ajouter la paire").clicked() && !nk.trim().is_empty() {
            list.push((nk.trim().to_string(), nv.clone()));
            changed = true;
            nk.clear();
            nv.clear();
        }
        set_temp_string(ui, kid, nk);
        set_temp_string(ui, vid, nv);
    });
    if changed {
        *value = Value::Array(list.into_iter().map(|(k, v)| Value::Array(vec![Value::String(k), Value::String(v)])).collect());
    }
    changed
}

// ------------------------------------------------------------------ listes

/// Liste JSON de valeurs (quêtes d'un groupe, groupes d'un point de téléportation, bonus…).
pub fn list(ui: &mut Ui, env: &mut Env, reference: Option<RefKind>, text: bool, value: &mut Value, id: Id) -> bool {
    let mut items: Vec<Value> = value.as_array().cloned().unwrap_or_default();
    let mut changed = false;
    if let Some(v) = env.modals.take_result(id.with("add")) {
        items.push(v);
        changed = true;
    }
    let mut remove = None;
    ui.horizontal_wrapped(|ui| {
        for (i, item) in items.iter().enumerate() {
            egui::Frame::new().fill(Color32::from_rgb(40, 40, 46)).corner_radius(10.0).inner_margin(egui::Margin::symmetric(8, 2)).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(if item.is_array() || item.is_object() { item.to_string() } else { display(item) }).monospace());
                    if let Some(r) = reference {
                        ref_name(ui, env, r, item);
                    }
                    if ui.small_button("×").on_hover_text("Retirer").clicked() {
                        remove = Some(i);
                    }
                });
            });
        }
        if items.is_empty() {
            ui.label(muted("liste vide"));
        }
    });
    if let Some(i) = remove {
        items.remove(i);
        changed = true;
    }
    ui.horizontal(|ui| {
        let nid = id.with("new");
        let mut nv = temp_string(ui, nid);
        let resp = ui.add(egui::TextEdit::singleline(&mut nv).hint_text("Ajouter…").desired_width(160.0));
        let enter = resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if (ui.button("Ajouter").clicked() || enter) && !nv.trim().is_empty() {
            let t = nv.trim();
            // Une valeur JSON ([1,2,3]…) s'ajoute telle quelle ; un nombre reste un nombre.
            let parsed = if text { Value::String(t.to_string()) } else { serde_json::from_str::<Value>(t).unwrap_or(Value::String(t.to_string())) };
            items.push(parsed);
            changed = true;
            nv.clear();
        }
        set_temp_string(ui, nid, nv);
        if let Some(r) = reference {
            if ui.button(format!("Choisir : {}…", ref_def(r).label)).clicked() {
                env.modals.open_picker(id.with("add"), r);
            }
        }
    });
    if changed {
        *value = Value::Array(items);
    }
    changed
}

/// Liste JSON de paires [TID, nombre] (matériaux, coûts, objets à charger, compétences exigées…).
pub fn pair_list(ui: &mut Ui, env: &mut Env, reference: Option<RefKind>, left: &str, right: &str, value: &mut Value, id: Id) -> bool {
    let mut items: Vec<(Value, Value)> = value
        .as_array()
        .map(|a| a.iter().filter_map(|p| p.as_array().map(|p| (p.first().cloned().unwrap_or(Value::from(0)), p.get(1).cloned().unwrap_or(Value::from(1))))).collect())
        .unwrap_or_default();
    let mut changed = false;
    let mut remove = None;
    let lf = match reference {
        Some(r) => f("tid", "").reference(r),
        None => f("tid", "").int(),
    };
    let rf = f("n", "").int();
    egui::Grid::new(id.with("pl")).striped(true).num_columns(3).show(ui, |ui| {
        ui.label(muted(left));
        ui.label(muted(right));
        ui.label("");
        ui.end_row();
        for (i, (a, b)) in items.iter_mut().enumerate() {
            changed |= value_input(ui, env, &lf, a, id.with(("l", i)), 90.0, true);
            changed |= value_input(ui, env, &rf, b, id.with(("r", i)), 70.0, true);
            if ui.small_button("×").clicked() {
                remove = Some(i);
            }
            ui.end_row();
        }
    });
    if let Some(i) = remove {
        items.remove(i);
        changed = true;
    }
    if ui.button("+ Ajouter une paire").clicked() {
        items.push((Value::from(0), Value::from(1)));
        changed = true;
    }
    if changed {
        *value = Value::Array(items.into_iter().map(|(a, b)| Value::Array(vec![a, b])).collect());
    }
    changed
}
