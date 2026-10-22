// Champs de l'éditeur. Chaque définition de champ :
//   { name, label, help, type, enum, flags, ref, readonly, nullable, server, wide }
// type : int, float, text, textarea, bool, enum, flags, datetime, json (objet clé → valeur),
//        pairs (liste de paires [clé, valeur] de npc.ini), list (liste JSON de nombres).
// `server: true` marque un champ lu par le serveur ArchCore (les autres ne servent qu'au client ou à
// l'import) ; `help` est l'explication affichée au survol.

import { h, clear, modal } from './ui.js';
import { REFS, resolveName, pickRef, refHref } from './refs.js';

export function parseValue(text, type, nullable) {
  const t = String(text).trim();
  if (t === '') return nullable ? null : type === 'int' || type === 'float' ? '' : '';
  if (type === 'int' && /^-?\d+$/.test(t)) {
    const n = Number(t);
    return Number.isSafeInteger(n) ? n : t;
  }
  if (type === 'float' && /^-?\d+(\.\d+)?([eE][-+]?\d+)?$/.test(t)) return Number(t);
  return type === 'int' || type === 'float' ? t : text;
}

export function isValid(value, def) {
  if (['json', 'pairs', 'list'].includes(def.type)) return true;
  if (value === null) return !!def.nullable;
  // Les tables du client laissent vides les colonnes sans valeur.
  if (value === '' && def.allowEmpty) return true;
  const numeric = ['int', 'enum', 'flags', 'bool'].includes(def.type) || (def.ref && !REFS[def.ref]?.text);
  if (numeric) return Number.isInteger(value) || /^-?\d+$/.test(String(value));
  if (def.type === 'float') return typeof value === 'number' || /^-?\d+(\.\d+)?([eE][-+]?\d+)?$/.test(String(value));
  return true;
}

function helpTip(def) {
  if (!def.help && !def.server) return null;
  return h(
    'span.help',
    { tabindex: 0 },
    def.server ? h('span.badge.server', { title: 'Lu par le serveur ArchCore' }, 'serveur') : null,
    def.help ? h('span.help-icon', {}, '?') : null,
    def.help ? h('span.help-text', {}, def.help) : null,
  );
}

export function fieldLabel(def) {
  return h('label.field-label', {}, h('span', {}, def.label ?? def.name), h('code.col', {}, def.name), helpTip(def));
}

// Nom de la référence, mis à jour quand la valeur change.
function refName(refKey, value) {
  const span = h('span.ref-name');
  const link = h('a.ref-link', { title: 'Ouvrir' }, '↗');
  const update = async (v) => {
    const href = refHref(refKey, v);
    link.style.display = href ? '' : 'none';
    if (href) link.href = href;
    span.textContent = '';
    if (v === '' || v === null || v === undefined || Number(v) === 0) return;
    const name = await resolveName(refKey, v);
    span.textContent = name ?? 'introuvable';
    span.classList.toggle('missing', name === null);
  };
  update(value);
  return { el: h('span.ref-info', {}, span, link), update };
}

function flagsSummary(def, value) {
  const n = Number(value) || 0;
  const on = def.flags.filter((f) => (n & f.bit) !== 0).map((f) => f.label);
  return on.length ? on.join(', ') : def.flagsNone ?? 'aucun';
}

export async function pickFlags(def, value) {
  let n = Number(value) || 0;
  const boxes = def.flags.map((f) =>
    h('label.check', {}, h('input', { type: 'checkbox', checked: (n & f.bit) !== 0, onchange: (e) => (n = e.target.checked ? n | f.bit : n & ~f.bit) }), ` ${f.label}`, h('code.col', {}, ` ${f.bit}`)),
  );
  return modal({
    title: def.label ?? def.name,
    body: h('div.flags', {}, def.help ? h('p.muted', {}, def.help) : null, boxes),
    buttons: [
      { label: 'Annuler', value: null },
      { label: 'Valider', kind: 'primary', onclick: () => n },
    ],
  });
}

// Champ simple (nombre, texte, liste, drapeaux, référence). onChange(valeur) à chaque saisie.
export function inputWidget(def, value, onChange, { compact = false } = {}) {
  const type = def.type ?? 'text';
  const readonly = !!def.readonly;
  const mark = (el, v) => el.classList.toggle('invalid', !isValid(v, def));

  if (type === 'bool') {
    return h('input', { type: 'checkbox', checked: Number(value) === 1, disabled: readonly, onchange: (e) => onChange(e.target.checked ? 1 : 0) });
  }

  if (type === 'enum') {
    const entries = Object.entries(def.enum);
    const known = entries.some(([k]) => String(k) === String(value));
    const sel = h(
      'select.input',
      { disabled: readonly, onchange: (e) => onChange(def.textEnum ? e.target.value : parseValue(e.target.value, 'int')) },
      !known ? h('option', { value: value ?? '', selected: true }, `${value ?? ''} (valeur inconnue)`) : null,
      entries.map(([k, label]) => h('option', { value: k, selected: String(k) === String(value) }, compact ? label : `${k} — ${label}`)),
    );
    return sel;
  }

  if (type === 'textarea') {
    return h('textarea.input', { rows: def.rows ?? 3, readOnly: readonly, value: value ?? '', oninput: (e) => onChange(def.nullable && e.target.value === '' ? null : e.target.value) });
  }

  const input = h('input.input', {
    type: 'text',
    value: value ?? '',
    readOnly: readonly,
    placeholder: value === null ? 'NULL' : '',
    inputmode: ['int', 'float', 'flags'].includes(type) || def.ref ? 'numeric' : undefined,
  });
  if (type === 'int' || type === 'float' || type === 'flags') input.classList.add('num');
  mark(input, value);

  if (def.ref) {
    const ref = refName(def.ref, value);
    input.addEventListener('input', () => {
      const v = REFS[def.ref].text ? input.value : parseValue(input.value, 'int', def.nullable);
      mark(input, v);
      ref.update(v);
      onChange(v);
    });
    const btn = h(
      'button.icon-btn',
      {
        title: `Choisir : ${REFS[def.ref].label}`,
        disabled: readonly,
        onclick: async () => {
          const v = await pickRef(def.ref, '');
          if (v === null || v === undefined) return;
          input.value = v;
          input.dispatchEvent(new Event('input'));
        },
      },
      '🔍',
    );
    return h('div.ref-field', {}, input, btn, ref.el);
  }

  if (type === 'flags') {
    const summary = h('span.flags-summary', {}, flagsSummary(def, value));
    input.addEventListener('input', () => {
      const v = parseValue(input.value, 'int', def.nullable);
      mark(input, v);
      summary.textContent = flagsSummary(def, v);
      onChange(v);
    });
    const btn = h(
      'button.icon-btn',
      {
        title: 'Choisir les drapeaux',
        disabled: readonly,
        onclick: async () => {
          const v = await pickFlags(def, input.value);
          if (v === null) return;
          input.value = v;
          input.dispatchEvent(new Event('input'));
        },
      },
      '⚑',
    );
    return h('div.ref-field', {}, input, btn, compact ? null : summary);
  }

  input.addEventListener('input', () => {
    const v = type === 'int' || type === 'float' ? parseValue(input.value, type, def.nullable) : def.nullable && input.value === '' ? null : input.value;
    mark(input, v);
    onChange(v);
  });
  return input;
}

// --- Colonne JSON objet (raw, data, spec…) : clés connues regroupées, puis toutes les autres ---
// def.sections : [{ title, keys: [{ key, label, help, type, ref, enum, server }] }]
export function jsonObjectWidget(def, value, onChange) {
  const obj = value && typeof value === 'object' && !Array.isArray(value) ? { ...value } : {};
  const emit = () => onChange({ ...obj });
  const known = new Set((def.sections ?? []).flatMap((s) => s.keys.map((k) => k.key)));
  const root = h('div.json-editor');

  for (const section of def.sections ?? []) {
    const grid = h('div.form-grid');
    for (const k of section.keys) {
      const kdef = { ...k, name: k.key, type: k.type ?? 'text', nullable: false, allowEmpty: true };
      // Les tables du client gardent tout en texte : la valeur repart en texte.
      const widget = inputWidget(kdef, obj[k.key] ?? '', (v) => {
        if (v === '' && !(k.key in (value ?? {}))) delete obj[k.key];
        else obj[k.key] = typeof v === 'number' && typeof (value ?? {})[k.key] !== 'number' ? String(v) : v;
        emit();
      });
      grid.append(h('div.field', { class: k.key in obj ? '' : 'absent' }, fieldLabel({ ...kdef, name: k.key, label: k.label ?? k.key }), widget));
    }
    root.append(h('fieldset.section', {}, h('legend', {}, section.title), grid));
  }

  // Autres clés : tableau filtrable, valeur modifiable, clé supprimable, ajout de clé.
  const filter = h('input.input', { type: 'search', placeholder: 'Filtrer les clés…' });
  const body = h('tbody');
  const renderOthers = () => {
    const f = filter.value.toLowerCase();
    clear(
      body,
      Object.keys(obj)
        .filter((k) => !known.has(k) && (!f || k.toLowerCase().includes(f) || String(obj[k]).toLowerCase().includes(f)))
        .map((k) =>
          h(
            'tr',
            {},
            h('td', {}, h('code', {}, k)),
            h(
              'td',
              {},
              typeof obj[k] === 'object' && obj[k] !== null
                ? h('code.muted', {}, JSON.stringify(obj[k]))
                : h('input.input', {
                    value: obj[k] ?? '',
                    oninput: (e) => {
                      obj[k] = typeof obj[k] === 'number' ? parseValue(e.target.value, 'float') : e.target.value;
                      emit();
                    },
                  }),
            ),
            h('td', {}, h('button.icon-btn', { title: 'Retirer la clé', onclick: () => { delete obj[k]; emit(); renderOthers(); } }, '✕')),
          ),
        ),
    );
  };
  filter.addEventListener('input', renderOthers);
  const newKey = h('input.input', { placeholder: 'Nouvelle clé' });
  const newVal = h('input.input', { placeholder: 'Valeur' });
  const add = h(
    'button.btn.small',
    {
      onclick: () => {
        const k = newKey.value.trim();
        if (!k) return;
        obj[k] = newVal.value;
        newKey.value = newVal.value = '';
        emit();
        renderOthers();
      },
    },
    'Ajouter',
  );
  renderOthers();
  const count = Object.keys(obj).filter((k) => !known.has(k)).length;
  root.append(
    h(
      'details.section.others',
      { open: !def.sections?.length },
      h('summary', {}, `${def.sections?.length ? 'Autres clés' : 'Clés'} (${count})`),
      h('div.toolbar', {}, filter),
      h('div.table-wrap.short', {}, h('table.grid.kv', {}, h('thead', {}, h('tr', {}, h('th', {}, 'Clé'), h('th', {}, 'Valeur'), h('th'))), body)),
      h('div.toolbar', {}, newKey, newVal, add),
    ),
  );
  return root;
}

// --- Paires [clé, valeur] de npc.ini : chaque « Function » ouvre un événement du PNJ ---
// Fonctions reconnues d'après leurs paramètres dans npc.ini (NPCTradeStart, NPCDialogStart, QuestGroupID,
// AuctionStart) ; les autres restent affichées par leur numéro.
const NPC_FUNCTIONS = { 9: 'Boutique', 21: 'Dialogue', 27: 'Quêtes', 28: 'Enchères' };
const PAIR_REFS = { QuestGroupID: 'quest_group', TID: 'creature' };

export function pairsWidget(def, value, onChange) {
  const pairs = Array.isArray(value) ? value.map((p) => [...p]) : [];
  const emit = () => onChange(pairs.map((p) => [...p]));
  const body = h('tbody');
  const render = () => {
    let fn = null;
    clear(
      body,
      pairs.map((p, i) => {
        if (p[0] === 'Function') fn = Number(p[1]);
        const isTemplate = p[0] === 'Template' && fn === 9;
        const refKey = isTemplate ? 'shop' : PAIR_REFS[p[0]];
        const vdef = { name: p[0], type: 'text', ref: refKey };
        const widget = inputWidget(vdef, p[1], (v) => {
          pairs[i][1] = String(v ?? '');
          emit();
          if (p[0] === 'Function') render();
        });
        return h(
          'tr',
          { class: p[0] === 'Function' ? 'event-start' : '' },
          h('td', {}, h('code', {}, p[0]), p[0] === 'Function' ? h('span.badge', {}, NPC_FUNCTIONS[Number(p[1])] ?? `fonction ${p[1]}`) : null),
          h('td', {}, widget),
          h('td', {}, h('button.icon-btn', { title: 'Retirer la paire', onclick: () => { pairs.splice(i, 1); emit(); render(); } }, '✕')),
        );
      }),
    );
  };
  render();
  const k = h('input.input', { placeholder: 'Clé (Function, Template…)' });
  const v = h('input.input', { placeholder: 'Valeur' });
  return h(
    'div.json-editor',
    {},
    def.help ? h('p.muted', {}, def.help) : null,
    h('div.table-wrap', {}, h('table.grid.kv', {}, h('thead', {}, h('tr', {}, h('th', {}, 'Clé'), h('th', {}, 'Valeur'), h('th'))), body)),
    h('div.toolbar', {}, k, v, h('button.btn.small', { onclick: () => { if (!k.value.trim()) return; pairs.push([k.value.trim(), v.value]); k.value = v.value = ''; emit(); render(); } }, 'Ajouter la paire')),
  );
}

// --- Liste JSON de nombres (quêtes d'un groupe, groupes d'un point de téléportation…) ---
export function listWidget(def, value, onChange) {
  const list = Array.isArray(value) ? [...value] : [];
  const box = h('div.chips');
  const emit = () => onChange([...list]);
  const render = () => {
    clear(
      box,
      list.map((item, i) => {
        const name = h('span.ref-name');
        if (def.ref) resolveName(def.ref, item).then((n) => (name.textContent = n ?? '?'));
        const href = def.ref ? refHref(def.ref, item) : null;
        return h('span.chip', {}, href ? h('a', { href }, String(item)) : String(item), ' ', name, h('button.chip-x', { title: 'Retirer', onclick: () => { list.splice(i, 1); emit(); render(); } }, '×'));
      }),
    );
  };
  render();
  const input = h('input.input.num', { placeholder: def.itemText ? 'Valeur' : 'Ajouter…' });
  const add = (v) => {
    const parsed = def.itemText ? String(v) : parseValue(String(v), 'int');
    if (parsed === '' || (!def.itemText && typeof parsed !== 'number')) return;
    list.push(parsed);
    emit();
    render();
  };
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Enter') {
      add(input.value);
      input.value = '';
    }
  });
  return h(
    'div.list-field',
    {},
    box,
    h(
      'div.toolbar',
      {},
      input,
      h('button.btn.small', { onclick: () => { add(input.value); input.value = ''; } }, 'Ajouter'),
      def.ref ? h('button.btn.small', { onclick: async () => { const v = await pickRef(def.ref); if (v !== null && v !== undefined) add(v); } }, `🔍 ${REFS[def.ref].label}`) : null,
    ),
  );
}

export function widgetFor(def, value, onChange) {
  if (def.type === 'json') return jsonObjectWidget(def, value, onChange);
  if (def.type === 'pairs') return pairsWidget(def, value, onChange);
  if (def.type === 'list') return listWidget(def, value, onChange);
  return inputWidget(def, value, onChange);
}
