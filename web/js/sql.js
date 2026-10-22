// Génération du SQL de l'éditeur (fonctions pures, testées par sql.test.mjs).
//
// Deux formes, comme Keira3 :
//   - « changements » : seulement ce qui diffère de la base (UPDATE ciblé, JSON_SET/JSON_REMOVE pour les
//     colonnes JSON, DELETE et INSERT des lignes retirées ou ajoutées) ;
//   - « entrée complète » : DELETE puis INSERT de toute l'entrée, pour une sauvegarde.
// Toutes les requêtes sont rejouables, comme l'exige la couche de contenu d'ArchCore.

export function ident(name) {
  return '`' + String(name).replace(/`/g, '``') + '`';
}

export function lit(value) {
  if (value === null || value === undefined) return 'NULL';
  if (typeof value === 'boolean') return value ? '1' : '0';
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new Error(`nombre invalide : ${value}`);
    return String(value);
  }
  if (typeof value === 'object') value = JSON.stringify(value);
  const escaped = String(value).replace(/[\0\n\r\x1a\\']/g, (c) =>
    ({ '\0': '\\0', '\n': '\\n', '\r': '\\r', '\x1a': '\\Z', '\\': '\\\\', "'": "\\'" })[c],
  );
  return `'${escaped}'`;
}

// Condition sur une clé ; une liste de valeurs donne un IN (textes de plusieurs sortes d'une entrée).
export function where(key) {
  return Object.entries(key)
    .map(([col, v]) =>
      v === null ? `${ident(col)} IS NULL` : Array.isArray(v) ? `${ident(col)} IN (${v.map(lit).join(', ')})` : `${ident(col)} = ${lit(v)}`,
    )
    .join(' AND ');
}

export function deepEqual(a, b) {
  if (a === b) return true;
  if (a === null || b === null || typeof a !== 'object' || typeof b !== 'object') {
    // 5 et '5' sont égaux pour la base : une saisie ne change pas une valeur qu'on n'a pas modifiée.
    if ((typeof a === 'number' && typeof b === 'string') || (typeof a === 'string' && typeof b === 'number')) {
      return String(a) === String(b);
    }
    return false;
  }
  if (Array.isArray(a) !== Array.isArray(b)) return false;
  const ka = Object.keys(a);
  const kb = Object.keys(b);
  if (ka.length !== kb.length) return false;
  if (Array.isArray(a)) return a.every((v, i) => deepEqual(v, b[i]));
  return ka.every((k) => Object.prototype.hasOwnProperty.call(b, k) && deepEqual(a[k], b[k]));
}

const isPlainObject = (v) => v !== null && typeof v === 'object' && !Array.isArray(v);

// Chemin JSON de MariaDB : $."clé avec espace"[3]
function jsonPath(parts) {
  return (
    '$' +
    parts.map((p) => (typeof p === 'number' ? `[${p}]` : `."${String(p).replace(/(["\\])/g, '\\$1')}"`)).join('')
  );
}

function jsonValue(v) {
  // Un objet ou une liste s'insère comme JSON, pas comme une chaîne.
  return v !== null && typeof v === 'object' ? `JSON_EXTRACT(${lit(v)}, '$')` : lit(v);
}

function diffJson(path, a, b, ops) {
  if (deepEqual(a, b)) return;
  if (isPlainObject(a) && isPlainObject(b)) {
    for (const k of Object.keys(a)) if (!(k in b)) ops.remove.push([...path, k]);
    for (const k of Object.keys(b)) {
      if (!(k in a)) ops.set.push([[...path, k], b[k]]);
      else diffJson([...path, k], a[k], b[k], ops);
    }
  } else if (Array.isArray(a) && Array.isArray(b) && a.length === b.length && path.length < 4) {
    b.forEach((v, i) => diffJson([...path, i], a[i], v, ops));
  } else {
    ops.set.push([path, b]);
  }
}

// Expression SQL qui fait passer la colonne JSON `col` de `before` à `after`, ou null si rien ne change.
// Les clés inchangées ne sont pas recopiées : un fichier de contenu ne reprend pas les données du client.
export function jsonPatch(col, before, after, maxOps = 40) {
  if (deepEqual(before, after)) return null;
  if (before === null || before === undefined || after === null || typeof before !== 'object') {
    return lit(after);
  }
  const ops = { set: [], remove: [] };
  diffJson([], before, after, ops);
  if (ops.set.some(([p]) => p.length === 0) || ops.set.length + ops.remove.length > maxOps) return lit(after);
  let expr = ident(col);
  if (ops.remove.length) expr = `JSON_REMOVE(${expr}, ${ops.remove.map((p) => lit(jsonPath(p))).join(', ')})`;
  if (ops.set.length) {
    expr = `JSON_SET(${expr}, ${ops.set.map(([p, v]) => `${lit(jsonPath(p))}, ${jsonValue(v)}`).join(', ')})`;
  }
  return expr;
}

// Valeur à écrire pour une colonne (les colonnes JSON sont gardées en objets dans l'éditeur).
function cell(row, col, jsonCols) {
  const v = row[col];
  return jsonCols.includes(col) && v !== null && v !== undefined ? lit(JSON.stringify(v)) : lit(v);
}

export function updateQuery(table, key, before, after, { columns, json = [] } = {}) {
  const cols = columns ?? Object.keys(after);
  const sets = [];
  for (const c of cols) {
    if (deepEqual(before[c], after[c])) continue;
    const expr = json.includes(c) ? jsonPatch(c, before[c], after[c]) : lit(after[c]);
    if (expr !== null) sets.push(`${ident(c)} = ${expr}`);
  }
  if (!sets.length) return '';
  return `UPDATE ${ident(table)} SET ${sets.join(', ')} WHERE ${where(key)};`;
}

export function insertQuery(table, rows, { columns, json = [], upsert = false, keyColumns = [] } = {}) {
  if (!rows.length) return '';
  const cols = columns ?? Object.keys(rows[0]);
  const values = rows.map((r) => `(${cols.map((c) => cell(r, c, json)).join(', ')})`);
  let sql = `INSERT INTO ${ident(table)} (${cols.map(ident).join(', ')}) VALUES\n${values.join(',\n')}`;
  if (upsert) {
    const updatable = cols.filter((c) => !keyColumns.includes(c));
    if (updatable.length) sql += `\nON DUPLICATE KEY UPDATE ${updatable.map((c) => `${ident(c)} = VALUES(${ident(c)})`).join(', ')}`;
  }
  return sql + ';';
}

// Insertion d'une ligne dont l'identifiant (`autoId`) n'est qu'un numéro : le prochain libre au moment où la
// requête passe. La clé stable de la ligne, unique, rend la requête rejouable (mise à jour au 2e passage).
export function insertAutoIdQuery(table, row, autoId, { columns, json = [], keyColumns = [] } = {}) {
  const cols = (columns ?? Object.keys(row)).filter((c) => c !== autoId);
  const t = ident(table);
  const updatable = cols.filter((c) => !keyColumns.includes(c));
  return (
    `INSERT INTO ${t} (${[autoId, ...cols].map(ident).join(', ')})\n` +
    `SELECT COALESCE(MAX(${ident(autoId)}), 0) + 1, ${cols.map((c) => cell(row, c, json)).join(', ')} FROM ${t}\n` +
    `ON DUPLICATE KEY UPDATE ${updatable.map((c) => `${ident(c)} = VALUES(${ident(c)})`).join(', ')};`
  );
}

export function deleteQuery(table, key) {
  return `DELETE FROM ${ident(table)} WHERE ${where(key)};`;
}

const pick = (row, cols) => Object.fromEntries(cols.map((c) => [c, row[c]]));

// --- Une ligne (onglet « une ligne » : modèle, objet, quête…) ---
// before : ligne en base (null si elle n'existe pas encore) ; key : colonnes de la clé primaire.
export function singleRowDiff(table, keyCols, before, after, opts = {}) {
  if (!before) return insertQuery(table, [after], { ...opts, upsert: true, keyColumns: keyCols });
  return updateQuery(table, pick(before, keyCols), before, after, opts);
}

export function singleRowFull(table, keyCols, after, opts = {}) {
  return `${deleteQuery(table, pick(after, keyCols))}\n${insertQuery(table, [after], opts)}`;
}

// --- Plusieurs lignes rattachées à une entrée (apparitions, butin, boutique…) ---
// rows : [{ before, after, deleted }] ; before null pour une ligne ajoutée.
// rowKey : colonnes qui désignent une ligne de façon stable (stable_key, ou clé primaire).
export function multiRowDiff(table, rowKey, rows, { autoId, ...opts } = {}) {
  const out = [];
  for (const r of rows) {
    if (r.deleted) {
      if (r.before) out.push(deleteQuery(table, pick(r.before, rowKey)));
    } else if (!r.before) {
      out.push(
        autoId
          ? insertAutoIdQuery(table, r.after, autoId, { ...opts, keyColumns: rowKey })
          : insertQuery(table, [r.after], { ...opts, upsert: true, keyColumns: rowKey }),
      );
    } else {
      const q = updateQuery(table, pick(r.before, rowKey), r.before, r.after, opts);
      if (q) out.push(q);
    }
  }
  return out.join('\n');
}

export function multiRowFull(table, parentKey, rows, { autoId, ...opts } = {}) {
  const kept = rows.filter((r) => !r.deleted);
  const existing = kept.filter((r) => r.before || !autoId).map((r) => r.after);
  const added = autoId ? kept.filter((r) => !r.before).map((r) => r.after) : [];
  const out = [deleteQuery(table, parentKey)];
  if (existing.length) out.push(insertQuery(table, existing, opts));
  for (const row of added) out.push(insertAutoIdQuery(table, row, autoId, opts));
  return out.join('\n');
}

// Instructions refusées par la couche de contenu (même règle que src/content.rs, côté serveur).
export function forbiddenStatements(sql) {
  const text = sql
    .replace(/'(?:[^'\\]|\\.|'')*'|"(?:[^"\\]|\\.)*"|`[^`]*`/g, ' ')
    .replace(/--[^\n]*|#[^\n]*|\/\*[\s\S]*?\*\//g, ' ')
    .toUpperCase();
  const words = ['CREATE', 'ALTER', 'DROP', 'TRUNCATE', 'RENAME', 'GRANT', 'REVOKE', 'START', 'COMMIT'];
  return words.filter((w) => new RegExp(`\\b${w}\\b`).test(text));
}
