// Références d'une table à l'autre (TID d'objet, de créature, de quête…) : nom affiché à côté du champ,
// fenêtre de recherche pour choisir la valeur (les « selectors » de Keira3), lien vers l'éditeur visé.

import { rows } from './api.js';
import { h, modal, clear, resultTable } from './ui.js';

// Nom d'une créature : celui de CharacterDataTable.txt, sauf s'il est illisible (texte coréen mal décodé à
// l'import, reconnaissable à « 占 ») ; sinon celui du modèle.
export const CREATURE_NAME = "COALESCE(IF(d.name LIKE '%占%', NULL, d.name), ct.name)";

export const REFS = {
  item: { label: 'Objet', db: 'world', from: 'item_data', key: 'tid', name: 'name', extra: { level: 'level' }, route: 'item' },
  creature: {
    label: 'Créature',
    db: 'world',
    from: 'character_template ct LEFT JOIN character_data_table d ON d.tid = ct.tid',
    key: 'ct.tid',
    name: CREATURE_NAME,
    extra: { niveau: "JSON_VALUE(d.data, '$.LVL')" },
    route: 'creature',
  },
  quest: { label: 'Quête', db: 'world', from: 'quest_template', key: 'tid', name: 'name', route: 'quest' },
  skill: { label: 'Compétence', db: 'world', from: 'skill_template', key: 'tid', name: 'name', route: 'skill' },
  npc: { label: 'PNJ', db: 'world', from: 'npc_spawn', key: 'id', name: 'name', route: 'npc' },
  shop: {
    label: 'Boutique',
    db: 'world',
    from: 'npc_trade_list',
    key: 'template_id',
    name: "CONCAT(COUNT(*), ' objets')",
    grouped: true,
    route: 'shop',
  },
  quest_group: { label: 'Groupe de quêtes', db: 'world', from: 'quest_group', key: 'id', name: 'name', route: 'quest-group' },
  region: { label: 'Région', db: 'world', from: 'region_template', key: 'id', name: 'name', route: 'region' },
  teleport_point: { label: 'Point de téléportation', db: 'world', from: 'teleport_point', key: 'name', name: 'name', text: true, route: 'teleport' },
  drop_group: { label: 'Groupe de butin', db: 'world', from: 'drop_group', key: 'group_id', name: 'name', route: 'drop-group' },
  language: { label: 'Langue', db: 'world', from: 'language', key: 'code', name: 'native_name', text: true },
  account: { label: 'Compte', db: 'auth', from: 'account', key: 'id', name: 'username', route: 'account' },
};

const cache = new Map(); // `${ref}:${id}` → Promise<string|null>
const pending = new Map(); // ref → { ids:Set, timer, waiters:Map }

function lookupSql(ref, count) {
  const marks = Array(count).fill('?').join(', ');
  return `SELECT ${ref.key} AS k, ${ref.name} AS n FROM ${ref.from} WHERE ${ref.key} IN (${marks})${ref.grouped ? ` GROUP BY ${ref.key}` : ''}`;
}

// Nom de l'entrée `id` de la référence `refName` ; les demandes rapprochées partent en une requête.
export function resolveName(refName, id) {
  const ref = REFS[refName];
  if (!ref || id === null || id === undefined || id === '') return Promise.resolve(null);
  const k = `${refName}:${id}`;
  if (cache.has(k)) return cache.get(k);
  const p = new Promise((resolve) => {
    let batch = pending.get(refName);
    if (!batch) {
      batch = { waiters: new Map() };
      pending.set(refName, batch);
      setTimeout(async () => {
        pending.delete(refName);
        const ids = [...batch.waiters.keys()];
        let found = new Map();
        try {
          const params = ids.map((i) => (ref.text ? String(i) : Number(i)));
          found = new Map((await rows(ref.db, lookupSql(ref, ids.length), params)).map((r) => [String(r.k), r.n]));
        } catch {
          /* table absente (ancienne base) : pas de nom */
        }
        for (const [i, waiters] of batch.waiters) for (const w of waiters) w(found.get(String(i)) ?? null);
      }, 30);
    }
    const list = batch.waiters.get(String(id)) ?? [];
    list.push(resolve);
    batch.waiters.set(String(id), list);
  });
  cache.set(k, p);
  return p;
}

export function forgetName(refName, id) {
  cache.delete(`${refName}:${id}`);
}

// Fenêtre de recherche : renvoie la clé choisie, ou null.
export function pickRef(refName, initial = '') {
  const ref = REFS[refName];
  const input = h('input.input', { type: 'search', placeholder: `${ref.text ? 'Nom' : 'TID ou nom'}…`, value: initial ?? '' });
  const results = h('div.picker-results');
  let choose;
  const search = async () => {
    const term = input.value.trim();
    const extra = Object.entries(ref.extra ?? {}).map(([alias, expr]) => `, ${expr} AS \`${alias}\``).join('');
    let sql = `SELECT ${ref.key} AS k, ${ref.name} AS n${extra} FROM ${ref.from}`;
    const params = [];
    if (term && /^\d+$/.test(term) && !ref.text) {
      sql += ` WHERE ${ref.key} = ?`;
      params.push(Number(term));
    } else if (term && !ref.grouped) {
      sql += ` WHERE ${ref.name} LIKE ?`;
      params.push(`%${term}%`);
    }
    if (ref.grouped) sql += ` GROUP BY ${ref.key}`;
    sql += ` ORDER BY ${ref.key} LIMIT 200`;
    try {
      const list = await rows(ref.db, sql, params);
      const cols = [{ name: 'k', label: ref.text ? 'Nom' : 'ID' }, { name: 'n', label: 'Nom' }, ...Object.keys(ref.extra ?? {}).map((a) => ({ name: a, label: a }))];
      clear(results, list.length ? resultTable(ref.text ? cols.slice(1) : cols, list, { onRow: (r) => choose(r.k) }) : h('p.muted', {}, 'Aucun résultat.'));
    } catch (e) {
      clear(results, h('p.error', {}, e.message));
    }
  };
  let timer;
  input.addEventListener('input', () => {
    clearTimeout(timer);
    timer = setTimeout(search, 250);
  });
  search();
  const dialog = modal({ title: `Choisir : ${ref.label}`, body: h('div.picker', {}, input, results), buttons: [{ label: 'Annuler', value: null }], wide: true });
  choose = (k) => dialog.close(k);
  return dialog;
}

// Lien vers l'éditeur de l'entrée référencée.
export function refHref(refName, id) {
  const ref = REFS[refName];
  return ref?.route && id !== null && id !== '' ? `#/e/${ref.route}/${encodeURIComponent(id)}` : null;
}
