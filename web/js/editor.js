// Moteur des éditeurs, sur le modèle des classes de base de Keira3 :
//   - SingleTab : une ligne d'une table (SingleRowEditorService) ;
//   - MultiTab  : les lignes rattachées à l'entrée (MultiRowEditorService) ;
//   - ListTab   : une liste en lecture seule (où l'objet est vendu, lâché…).
// Chaque onglet produit ses requêtes (changements et entrée complète) ; la sortie SQL les affiche, les
// copie, les exécute sur la base ou les ajoute à la couche de contenu d'ArchCore.

import { api, rows } from './api.js';
import { h, clear, toast, modal, highlightSql, copyText, resultTable, store, formatCell } from './ui.js';
import * as SQL from './sql.js';
import { widgetFor, inputWidget, fieldLabel, isValid } from './fields.js';
import { resolveName, forgetName, refHref } from './refs.js';

const clone = (v) => (v === undefined ? undefined : JSON.parse(JSON.stringify(v)));

function parseJson(row, cols = []) {
  const out = { ...row };
  for (const c of cols) {
    if (typeof out[c] === 'string') {
      try {
        out[c] = JSON.parse(out[c]);
      } catch {
        /* JSON invalide en base : laissé en texte */
      }
    }
  }
  return out;
}

function whereParams(key) {
  const sql = Object.entries(key)
    .map(([c, v]) => (Array.isArray(v) ? `${SQL.ident(c)} IN (${v.map(() => '?').join(', ')})` : `${SQL.ident(c)} = ?`))
    .join(' AND ');
  return { sql, params: Object.values(key).flat() };
}

// Colonnes de rattachement à recopier dans une ligne (une liste de valeurs n'en fixe aucune).
const attach = (key) => Object.fromEntries(Object.entries(key).filter(([, v]) => !Array.isArray(v)));

// ------------------------------------------------------------------ onglets

class Tab {
  constructor(def, editor) {
    this.def = def;
    this.editor = editor;
    this.db = def.db ?? editor.entity.db ?? 'world';
  }
  get label() {
    return this.def.label;
  }
  get dirty() {
    try {
      return !!this.diffSql();
    } catch {
      return true;
    }
  }
  errors() {
    return [];
  }
  diffSql() {
    return '';
  }
  fullSql() {
    return '';
  }
  changed() {
    this.editor.onTabChanged(this);
  }
}

class SingleTab extends Tab {
  get keyCols() {
    return Object.keys(this.def.key(this.editor.id));
  }

  async fetchRow(id) {
    const w = whereParams(this.def.key(id));
    const r = await rows(this.db, `SELECT * FROM ${SQL.ident(this.def.table)} WHERE ${w.sql}`, w.params);
    return r[0] ? parseJson(r[0], this.def.json) : null;
  }

  async load() {
    const { id, isNew, copyFrom } = this.editor;
    this.before = isNew ? null : await this.fetchRow(id);
    this.after = clone(this.before);
    if (isNew) {
      const source = copyFrom !== null && copyFrom !== undefined ? await this.fetchRow(copyFrom) : null;
      if (source) {
        const row = { ...source, ...this.def.key(id) };
        this.after = this.def.onCopy ? this.def.onCopy(row, id) : row;
      }
      else if (this.def.main) this.after = { ...this.def.key(id), ...(this.def.defaults?.(id) ?? {}) };
    }
  }

  createRow() {
    this.after = { ...this.def.key(this.editor.id), ...(this.def.defaults?.(this.editor.id) ?? {}) };
    this.changed();
    this.editor.renderActive();
  }

  errors() {
    if (!this.after) return [];
    return this.def.fields.filter((f) => !f.readonly && !isValid(this.after[f.name], f)).map((f) => f.label ?? f.name);
  }

  diffSql() {
    if (!this.after) return '';
    return SQL.singleRowDiff(this.def.table, this.keyCols, this.before, this.after, { json: this.def.json ?? [] });
  }

  fullSql() {
    if (!this.after) return '';
    return SQL.singleRowFull(this.def.table, this.keyCols, this.after, { json: this.def.json ?? [] });
  }

  render() {
    if (!this.after) {
      return h(
        'div.empty-state',
        {},
        h('p', {}, `Aucune ligne dans ${this.def.table} pour cette entrée.`),
        this.def.help ? h('p.muted', {}, this.def.help) : null,
        h('button.btn.primary', { onclick: () => this.createRow() }, 'Créer la ligne'),
      );
    }
    const blocks = [];
    let grid = null;
    let groupName;
    for (const f of this.def.fields) {
      const big = ['json', 'pairs', 'list', 'textarea'].includes(f.type) || f.wide;
      if (big) {
        grid = null;
        const widget = widgetFor({ ...f, readonly: f.readonly || this.keyCols.includes(f.name) }, this.after[f.name], (v) => {
          this.after[f.name] = v;
          this.changed();
        });
        blocks.push(h('div.field.wide', {}, f.type === 'json' || f.type === 'pairs' ? h('h4', {}, f.label ?? f.name, h('code.col', {}, ` ${f.name}`)) : fieldLabel(f), widget));
        continue;
      }
      if (!grid || f.group !== groupName) {
        groupName = f.group;
        grid = h('div.form-grid');
        blocks.push(f.group ? h('fieldset.section', {}, h('legend', {}, f.group), grid) : grid);
      }
      const readonly = f.readonly || this.keyCols.includes(f.name);
      grid.append(
        h(
          'div.field',
          {},
          fieldLabel(f),
          inputWidget({ ...f, readonly }, this.after[f.name], (v) => {
            this.after[f.name] = v;
            this.changed();
          }),
        ),
      );
    }
    return h('div.single-tab', {}, this.def.help ? h('p.tab-help', {}, this.def.help) : null, blocks);
  }
}

class MultiTab extends Tab {
  parentKey(id = this.editor.id) {
    return this.def.parent(id);
  }

  async fetchRows(id) {
    const w = whereParams(this.parentKey(id));
    const order = this.def.order ? ` ORDER BY ${this.def.order}` : '';
    const r = await api.select(this.db, `SELECT * FROM ${SQL.ident(this.def.table)} WHERE ${w.sql}${order}`, w.params);
    this.columns = r.columns;
    return r.rows.map((row) => parseJson(Object.fromEntries(r.columns.map((c, i) => [c, row[i]])), this.def.json));
  }

  async load() {
    const { id, isNew, copyFrom } = this.editor;
    const loaded = isNew ? [] : await this.fetchRows(id);
    this.rows = loaded.map((r) => ({ before: r, after: clone(r), deleted: false }));
    if (isNew && this.def.copy && copyFrom !== null && copyFrom !== undefined) {
      const source = await this.fetchRows(copyFrom);
      this.rows = source.map((r, i) => ({ before: null, after: this.newRowFrom(r, i), deleted: false }));
    }
    if (isNew && !this.columns) await this.fetchRows(id);
  }

  // Ligne neuve tirée d'une autre (copie, duplication) : rattachée à l'entrée, clés régénérées.
  newRowFrom(row, i = 0) {
    const r = { ...clone(row), ...attach(this.parentKey()) };
    if (this.def.autoId) r[this.def.autoId] = null;
    if (this.def.autoIncrement) delete r[this.def.autoIncrement];
    return this.def.fresh ? this.def.fresh(r, this, i) : r;
  }

  addRow() {
    const last = this.rows.filter((r) => !r.deleted).at(-1)?.after;
    const base = this.def.newRow ? this.def.newRow(this.editor.id, this, last) : last ? this.newRowFrom(last) : attach(this.parentKey());
    this.rows.push({ before: null, after: this.def.fresh && this.def.newRow ? this.def.fresh(base, this, this.rows.length) : base, deleted: false });
    this.changed();
    this.editor.renderActive();
  }

  errors() {
    const errs = [];
    this.rows.forEach((r, i) => {
      if (r.deleted) return;
      for (const f of this.def.columns) if (!f.readonly && !isValid(r.after[f.name], f)) errs.push(`ligne ${i + 1} : ${f.label ?? f.name}`);
    });
    const missing = (row) => this.def.rowKey.some((k) => row[k] === null || row[k] === undefined || row[k] === '');
    // Une ligne sans clé ne peut être visée (UPDATE ou DELETE toucheraient d'autres lignes).
    this.rows.forEach((r, i) => {
      if (r.before && missing(r.before) && this.rowState(r)) errs.push(`ligne ${i + 1} : clé ${this.def.rowKey.join(', ')} vide en base`);
    });
    const keys = this.rows.filter((r) => !r.deleted && !missing(r.after)).map((r) => this.def.rowKey.map((k) => r.after[k]).join('\u0001'));
    if (new Set(keys).size !== keys.length) errs.push(`deux lignes ont la même clé (${this.def.rowKey.join(', ')})`);
    return errs;
  }

  opts() {
    return { autoId: this.def.autoId, json: this.def.json ?? [] };
  }

  diffSql() {
    return SQL.multiRowDiff(this.def.table, this.def.rowKey, this.rows, this.opts());
  }

  fullSql() {
    return SQL.multiRowFull(this.def.table, this.parentKey(), this.rows, this.opts());
  }

  rowState(r) {
    if (r.deleted) return 'deleted';
    if (!r.before) return 'added';
    return SQL.deepEqual(r.before, r.after) ? '' : 'changed';
  }

  render() {
    const cols = this.def.columns;
    const body = h('tbody');
    const live = this.rows.filter((r) => !r.deleted).length;
    this.rows.forEach((r, i) => {
      const cells = cols.map((f) => {
        const readonly = f.readonly || r.deleted || (r.before && this.def.rowKey.includes(f.name) && f.lockKey !== false);
        if (['json', 'list', 'pairs'].includes(f.type)) {
          const preview = h('code.muted', {}, JSON.stringify(r.after[f.name] ?? null).slice(0, 60));
          return h(
            'td',
            {},
            h('button.btn.small', {
              disabled: readonly,
              onclick: async () => {
                let value = clone(r.after[f.name]);
                const ok = await modal({ title: `${f.label ?? f.name} — ligne ${i + 1}`, body: widgetFor(f, value, (v) => (value = v)), buttons: [{ label: 'Annuler', value: false }, { label: 'Valider', kind: 'primary', value: true }], wide: true });
                if (!ok) return;
                r.after[f.name] = value;
                this.changed();
                this.editor.renderActive();
              },
            }, 'Modifier'),
            ' ',
            preview,
          );
        }
        const cellChanged = r.before && !SQL.deepEqual(r.before[f.name], r.after[f.name]);
        return h(
          'td',
          { class: cellChanged ? 'cell-changed' : '' },
          inputWidget({ ...f, readonly }, r.after[f.name], (v) => {
            r.after[f.name] = v;
            this.changed();
            const tr = body.children[i];
            tr.className = this.rowState(r);
          }, { compact: true }),
        );
      });
      body.append(
        h(
          'tr',
          { class: this.rowState(r) },
          h('td.row-num', {}, i + 1),
          cells,
          h(
            'td.row-actions',
            {},
            r.deleted
              ? h('button.icon-btn', { title: 'Restaurer', onclick: () => { r.deleted = false; this.changed(); this.editor.renderActive(); } }, '↺')
              : [
                  h('button.icon-btn', { title: 'Dupliquer', onclick: () => { this.rows.splice(i + 1, 0, { before: null, after: this.newRowFrom(r.after, i + 1), deleted: false }); this.changed(); this.editor.renderActive(); } }, '⧉'),
                  h('button.icon-btn', {
                    title: 'Supprimer',
                    onclick: () => {
                      if (r.before) r.deleted = true;
                      else this.rows.splice(i, 1);
                      this.changed();
                      this.editor.renderActive();
                    },
                  }, '✕'),
                ],
          ),
        ),
      );
    });
    return h(
      'div.multi-tab',
      {},
      this.def.help ? h('p.tab-help', {}, this.def.help) : null,
      h('div.toolbar', {}, h('button.btn.primary', { onclick: () => this.addRow() }, '+ Ajouter une ligne'), h('span.muted', {}, `${live} ligne(s) — clé : ${this.def.rowKey.join(', ')}`)),
      h(
        'div.table-wrap.tall',
        {},
        h('table.grid.edit', {}, h('thead', {}, h('tr', {}, h('th', {}, '#'), cols.map((f) => h('th', {}, fieldLabel(f))), h('th'))), body),
      ),
    );
  }
}

class ListTab extends Tab {
  async load() {
    if (this.editor.isNew) {
      this.data = [];
      return;
    }
    const [sql, params] = this.def.query(this.editor.id);
    this.data = await rows(this.db, sql, params);
  }

  render() {
    if (!this.data.length) return h('div.empty-state', {}, h('p', {}, this.def.empty ?? 'Aucune ligne.'));
    const cols = this.def.columns;
    return h(
      'div',
      {},
      this.def.help ? h('p.tab-help', {}, this.def.help) : null,
      resultTable(cols, this.data, {
        render: (c, v) => {
          if (!c.ref) return formatCell(v);
          const name = h('span.ref-name');
          resolveName(c.ref, v).then((n) => (name.textContent = n ?? ''));
          const href = refHref(c.ref, v);
          return h('span', {}, href ? h('a', { href }, String(v)) : String(v), ' ', name);
        },
      }),
    );
  }
}

const TAB_KINDS = { single: SingleTab, multi: MultiTab, list: ListTab };

// ------------------------------------------------------------------ sortie SQL

function contentDialog(title, sql, { full }) {
  return (async () => {
    let files = [];
    try {
      files = await api.contentList();
    } catch (e) {
      toast(e.message, 'error', 6000);
      return null;
    }
    const forbidden = SQL.forbiddenStatements(sql);
    const select = h('select.input', {}, h('option', { value: '' }, '— Nouveau fichier —'), files.map((f) => h('option', { value: f.name }, f.name)));
    const subject = h('input.input', { value: title, placeholder: 'Sujet du nouveau fichier' });
    const label = h('input.input', { value: title });
    select.addEventListener('change', () => (subject.disabled = !!select.value));
    const body = h(
      'div',
      {},
      h('p.muted', {}, 'Les fichiers de content/ sont rejoués par `archcore import` et `archcore content`, chacun dans une transaction.'),
      full ? h('p.warning', {}, "Entrée complète : elle recopie toutes les valeurs, y compris celles importées du client. La couche de contenu ne doit contenir que des valeurs créées pour ArchCore ; préférer « Changements » pour une entrée importée.") : null,
      forbidden.length ? h('p.error', {}, `Refusé par la couche de contenu : ${forbidden.join(', ')}.`) : null,
      h('div.form-grid', {}, h('div.field', {}, h('label.field-label', {}, 'Fichier'), select), h('div.field', {}, h('label.field-label', {}, 'Sujet (nouveau fichier)'), subject), h('div.field.wide', {}, h('label.field-label', {}, 'Commentaire du bloc'), label)),
      h('pre.sql', { html: highlightSql(sql) }),
    );
    return modal({
      title: 'Ajouter à la couche de contenu',
      body,
      wide: true,
      buttons: [
        { label: 'Annuler', value: null },
        {
          label: 'Ajouter',
          kind: 'primary',
          onclick: async () => {
            try {
              const r = await api.contentAppend({ file: select.value || null, subject: subject.value, title: label.value || title, sql });
              toast(`Ajouté à content/${r.name}`, 'success');
              return r;
            } catch (e) {
              toast(e.message, 'error', 8000);
              return false;
            }
          },
        },
      ],
    });
  })();
}

function queryOutput(editor, tab) {
  let mode = store.get('queryMode', 'diff');
  const pre = h('pre.sql');
  const status = h('span.muted');
  const btnDiff = h('button.seg', { onclick: () => setMode('diff') }, 'Changements');
  const btnFull = h('button.seg', { onclick: () => setMode('full') }, 'Entrée complète');
  const copy = h('button.btn', { onclick: () => copyText(current()) }, 'Copier');
  const run = h('button.btn.primary', { onclick: () => editor.executeTabs([tab]) }, 'Exécuter');
  const content = h('button.btn', { onclick: () => contentDialog(`${editor.titleText()} — ${tab.label}`, current(), { full: mode === 'full' }) }, 'Ajouter au contenu');
  const current = () => (mode === 'diff' ? tab.diffSql() : tab.fullSql());
  const setMode = (m) => {
    mode = m;
    store.set('queryMode', m);
    refresh();
  };
  const refresh = () => {
    btnDiff.classList.toggle('active', mode === 'diff');
    btnFull.classList.toggle('active', mode === 'full');
    let sql = '';
    let error = null;
    try {
      sql = current();
    } catch (e) {
      error = e.message;
    }
    const errors = tab.errors();
    pre.innerHTML = sql ? highlightSql(sql) : `<span class="muted">${mode === 'diff' ? '-- Aucun changement.' : '-- Entrée vide.'}</span>`;
    status.textContent = error ?? (errors.length ? `À corriger : ${errors.join(' ; ')}` : '');
    status.className = error || errors.length ? 'error' : 'muted';
    const blocked = !sql || !!error || errors.length > 0;
    run.disabled = blocked;
    copy.disabled = !sql;
    content.disabled = blocked || tab.db !== 'world';
    content.title = tab.db !== 'world' ? 'La couche de contenu ne vise que la base monde' : '';
  };
  refresh();
  return {
    el: h('div.query-output', {}, h('div.query-head', {}, h('div.segmented', {}, btnDiff, btnFull), status, h('div.spacer'), copy, content, run), pre),
    refresh,
  };
}

// ------------------------------------------------------------------ éditeur d'une entrée

export class EntityEditor {
  constructor(entity, id, { isNew = false, copyFrom = null } = {}) {
    this.entity = entity;
    this.id = id;
    this.isNew = isNew;
    this.copyFrom = copyFrom;
    this.tabs = entity.tabs.map((def) => new TAB_KINDS[def.kind](def, this));
    this.active = 0;
  }

  get dirty() {
    return this.tabs.some((t) => t.dirty);
  }

  titleText() {
    return `${this.entity.singular} ${this.id}`;
  }

  async mount(container) {
    this.container = container;
    clear(container, h('p.muted', {}, 'Chargement…'));
    try {
      await Promise.all(this.tabs.map((t) => t.load()));
    } catch (e) {
      clear(container, h('p.error', {}, e.message));
      return;
    }
    const wanted = new URLSearchParams(location.hash.split('?')[1] ?? '').get('tab');
    const idx = this.tabs.findIndex((t) => t.def.id === wanted);
    if (idx >= 0) this.active = idx;
    this.render();
  }

  render() {
    const nameEl = h('span.entry-name');
    if (this.entity.ref && !this.isNew) resolveName(this.entity.ref, this.id).then((n) => (nameEl.textContent = n ?? ''));
    this.nav = h('nav.tabs');
    this.panel = h('div.tab-panel');
    this.saveAll = h('button.btn.primary', { onclick: () => this.executeTabs(this.tabs.filter((t) => t.dirty)) }, 'Tout enregistrer');
    this.contentAll = h('button.btn', { onclick: () => this.contentAllDialog() }, 'Tout ajouter au contenu');
    clear(
      this.container,
      h(
        'header.page-head',
        {},
        h('div', {}, h('a.crumb', { href: `#/e/${this.entity.route}` }, this.entity.label), h('h2', {}, this.titleText(), ' ', nameEl, this.isNew ? h('span.badge.new', {}, this.copyFrom !== null ? `copie de ${this.copyFrom}` : 'nouvelle') : null)),
        h('div.actions', {}, h('button.btn', { onclick: () => this.reload() }, 'Recharger'), this.contentAll, this.saveAll),
      ),
      this.entity.help ? h('p.tab-help', {}, this.entity.help) : null,
      this.nav,
      this.panel,
    );
    this.renderNav();
    this.renderActive();
  }

  renderNav() {
    clear(
      this.nav,
      this.tabs.map((t, i) =>
        h('button.tab', { class: i === this.active ? 'active' : '', onclick: () => { this.active = i; this.renderNav(); this.renderActive(); } }, t.label, t.dirty ? h('span.dot', { title: 'Modifications non enregistrées' }) : null, t.def.kind === 'multi' ? h('span.count', {}, t.rows.filter((r) => !r.deleted).length) : null),
      ),
    );
    const dirtyTabs = this.tabs.filter((t) => t.dirty);
    this.saveAll.disabled = !dirtyTabs.length || dirtyTabs.some((t) => t.errors().length);
    this.contentAll.disabled = !dirtyTabs.some((t) => t.db === 'world');
  }

  renderActive() {
    const tab = this.tabs[this.active];
    this.output = tab.def.kind === 'list' ? null : queryOutput(this, tab);
    clear(this.panel, tab.render(), this.output?.el);
    this.renderNav();
  }

  onTabChanged(tab) {
    clearTimeout(this.timer);
    this.timer = setTimeout(() => {
      if (this.tabs[this.active] === tab) this.output?.refresh();
      this.renderNav();
    }, 120);
  }

  async reload() {
    if (this.dirty && !(await modal({ title: 'Recharger', body: h('p', {}, 'Abandonner les modifications non enregistrées ?'), buttons: [{ label: 'Annuler', value: false }, { label: 'Abandonner', kind: 'danger', value: true }] }))) return;
    await this.mount(this.container);
  }

  async executeTabs(tabs) {
    const byDb = new Map();
    for (const t of tabs) {
      const sql = t.diffSql();
      if (sql) byDb.set(t.db, [...(byDb.get(t.db) ?? []), sql]);
    }
    if (!byDb.size) return;
    try {
      for (const [db, parts] of byDb) await api.execute(db, parts.join('\n'));
    } catch (e) {
      toast(`Échec, rien n'a été écrit : ${e.message}`, 'error', 10000);
      return;
    }
    toast('Requête exécutée. Redémarrer le serveur ArchCore pour qu’il relise le monde.', 'success', 6000);
    if (this.entity.ref) forgetName(this.entity.ref, this.id);
    if (this.isNew) {
      this.isNew = false;
      this.copyFrom = null;
      history.replaceState(null, '', `#/e/${this.entity.route}/${encodeURIComponent(this.id)}`);
      rememberRecent(this.entity, this.id);
    }
    const tabId = this.tabs[this.active].def.id;
    await Promise.all(this.tabs.map((t) => t.load()));
    this.active = Math.max(0, this.tabs.findIndex((t) => t.def.id === tabId));
    this.render();
  }

  async contentAllDialog() {
    const sql = this.tabs
      .filter((t) => t.dirty && t.db === 'world')
      .map((t) => `-- ${t.label}\n${t.diffSql()}`)
      .join('\n\n');
    if (sql) await contentDialog(this.titleText(), sql, { full: false });
  }
}

// ------------------------------------------------------------------ recherche et création

export function rememberRecent(entity, id) {
  const key = `recent.${entity.route}`;
  const list = store.get(key, []).filter((x) => String(x) !== String(id));
  store.set(key, [id, ...list].slice(0, 12));
}

export async function searchPage(container, entity) {
  const s = entity.search;
  const saved = store.get(`filters.${entity.route}`, {});
  const inputs = {};
  const form = h('div.filters');
  for (const f of s.filters) {
    const el = f.enum
      ? h('select.input', {}, h('option', { value: '' }, 'Tous'), Object.entries(f.enum).map(([k, v]) => h('option', { value: k, selected: String(saved[f.name] ?? '') === String(k) }, v)))
      : h('input.input', { type: 'search', value: saved[f.name] ?? '', placeholder: f.placeholder ?? '' });
    el.addEventListener(f.enum ? 'change' : 'input', () => schedule());
    inputs[f.name] = el;
    form.append(h('div.field', {}, h('label.field-label', {}, f.label), el));
  }
  const results = h('div.results');
  const pager = h('div.pager');
  let page = 0;
  const PAGE = 50;
  let timer;
  const schedule = () => {
    clearTimeout(timer);
    timer = setTimeout(() => {
      page = 0;
      search();
    }, 250);
  };

  const search = async () => {
    const where = [];
    const params = [];
    const values = {};
    for (const f of s.filters) {
      const v = inputs[f.name].value.trim();
      values[f.name] = v;
      if (!v) continue;
      if (f.op === 'like') {
        where.push(`${f.expr} LIKE ?`);
        params.push(`%${v}%`);
      } else if (f.op === 'having') {
        continue;
      } else {
        where.push(`${f.expr} = ?`);
        params.push(/^-?\d+$/.test(v) && !f.text ? Number(v) : v);
      }
    }
    store.set(`filters.${entity.route}`, values);
    const cols = s.columns.map((c) => `${c.expr} AS ${SQL.ident(c.name)}`).join(', ');
    const sql = `SELECT ${cols} FROM ${s.from}${where.length ? ` WHERE ${where.join(' AND ')}` : ''}${s.groupBy ? ` GROUP BY ${s.groupBy}` : ''} ORDER BY ${s.order} LIMIT ${PAGE + 1} OFFSET ${page * PAGE}`;
    try {
      const list = await rows(entity.db ?? 'world', sql, params);
      const more = list.length > PAGE;
      clear(
        results,
        list.length
          ? resultTable(s.columns, list.slice(0, PAGE), {
              onRow: (r) => (location.hash = `#/e/${entity.route}/${encodeURIComponent(r[entity.key.column])}`),
              render: (c, v) => (c.enum ? (c.enum[v] ?? formatCell(v)) : formatCell(v)),
            })
          : h('p.muted', {}, 'Aucun résultat.'),
      );
      clear(
        pager,
        h('button.btn.small', { disabled: page === 0, onclick: () => { page--; search(); } }, '‹ Précédent'),
        h('span.muted', {}, `Page ${page + 1}`),
        h('button.btn.small', { disabled: !more, onclick: () => { page++; search(); } }, 'Suivant ›'),
      );
    } catch (e) {
      clear(results, h('p.error', {}, e.message));
    }
  };

  clear(
    container,
    h('header.page-head', {}, h('div', {}, h('h2', {}, entity.label), entity.help ? h('p.muted', {}, entity.help) : null)),
    entity.create === false ? null : createPanel(entity),
    recentPanel(entity),
    h('section.card', {}, h('h3', {}, 'Rechercher'), form, results, pager),
  );
  search();
}

function recentPanel(entity) {
  const list = store.get(`recent.${entity.route}`, []);
  if (!list.length) return null;
  return h(
    'section.card.recent',
    {},
    h('h3', {}, 'Récents'),
    h(
      'div.chips',
      {},
      list.map((id) => {
        const name = h('span.ref-name');
        if (entity.ref) resolveName(entity.ref, id).then((n) => (name.textContent = n ?? ''));
        return h('a.chip', { href: `#/e/${entity.route}/${encodeURIComponent(id)}` }, String(id), ' ', name);
      }),
    ),
  );
}

function createPanel(entity) {
  const idInput = h('input.input', { placeholder: entity.key.text ? 'Nom' : 'Nouvel identifiant' });
  const copyInput = h('input.input', { placeholder: 'facultatif' });
  if (entity.create?.nextId) {
    rows(entity.db ?? 'world', entity.create.nextId).then((r) => (idInput.value = r[0]?.n ?? '')).catch(() => {});
  }
  const create = async () => {
    const id = idInput.value.trim();
    if (!id) return toast('Indiquer un identifiant', 'error');
    if (!entity.key.text && !/^\d+$/.test(id)) return toast('Identifiant numérique attendu', 'error');
    const exists = await rows(entity.db ?? 'world', `SELECT 1 AS x FROM ${SQL.ident(entity.table)} WHERE ${SQL.ident(entity.key.column)} = ? LIMIT 1`, [entity.key.text ? id : Number(id)]);
    if (exists.length) return toast(`${entity.singular} ${id} existe déjà`, 'error');
    const copy = copyInput.value.trim();
    location.hash = `#/e/${entity.route}/${encodeURIComponent(id)}?new=1${copy ? `&copy=${encodeURIComponent(copy)}` : ''}`;
  };
  const pick = entity.ref
    ? h('button.icon-btn', { title: 'Choisir l’entrée à copier', onclick: async () => { const { pickRef } = await import('./refs.js'); const v = await pickRef(entity.ref); if (v !== null && v !== undefined) copyInput.value = v; } }, '🔍')
    : null;
  return h(
    'section.card',
    {},
    h('h3', {}, `Nouvelle entrée`),
    h(
      'div.filters',
      {},
      h('div.field', {}, h('label.field-label', {}, entity.key.label), idInput),
      h('div.field', {}, h('label.field-label', {}, 'Copier depuis'), h('div.ref-field', {}, copyInput, pick)),
      h('div.field', {}, h('label.field-label', {}, ' '), h('button.btn.primary', { onclick: create }, 'Créer')),
    ),
  );
}
