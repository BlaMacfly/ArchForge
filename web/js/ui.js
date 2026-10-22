// Petits outils d'interface : création d'éléments, fenêtres, notifications, coloration du SQL.

export function h(tag, attrs = {}, ...children) {
  const [name, ...classes] = tag.split('.');
  const el = document.createElement(name || 'div');
  if (classes.length) el.className = classes.join(' ');
  for (const [k, v] of Object.entries(attrs ?? {})) {
    if (v === undefined || v === null || v === false) continue;
    if (k === 'class') el.className = [el.className, v].filter(Boolean).join(' ');
    else if (k === 'style' && typeof v === 'object') Object.assign(el.style, v);
    else if (k.startsWith('on') && typeof v === 'function') el.addEventListener(k.slice(2), v);
    else if (k === 'html') el.innerHTML = v;
    else if (k in el && typeof v !== 'string') el[k] = v;
    else el.setAttribute(k, v === true ? '' : v);
  }
  append(el, children);
  return el;
}

function append(el, children) {
  for (const c of children.flat(Infinity)) {
    if (c === null || c === undefined || c === false) continue;
    el.append(c instanceof Node ? c : document.createTextNode(String(c)));
  }
}

export function clear(el, ...children) {
  el.replaceChildren();
  append(el, children);
  return el;
}

export function escapeHtml(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
}

const KEYWORDS = new Set(
  ('SELECT FROM WHERE AND OR NOT NULL IS IN INSERT INTO VALUES UPDATE SET DELETE ON DUPLICATE KEY ORDER BY ' +
    'GROUP LIMIT OFFSET JOIN LEFT RIGHT INNER AS DISTINCT COUNT MAX MIN COALESCE LIKE BETWEEN CASE WHEN THEN ELSE ' +
    'END CREATE TABLE ALTER DROP START TRANSACTION COMMIT ROLLBACK USE SHOW DESC ASC HAVING UNION ALL EXISTS ' +
    'JSON_SET JSON_REMOVE JSON_EXTRACT JSON_VALUE REPLACE').split(' '),
);

// Coloration du SQL : renvoie du HTML (le texte est échappé).
export function highlightSql(sql) {
  const re = /(--[^\n]*|#[^\n]*)|('(?:[^'\\]|\\.|'')*')|(`[^`]*`)|(\b\d+(?:\.\d+)?\b)|([A-Za-z_][A-Za-z0-9_]*)/g;
  let out = '';
  let last = 0;
  for (const m of sql.matchAll(re)) {
    out += escapeHtml(sql.slice(last, m.index));
    const [t, comment, str, quoted, num, word] = m;
    if (comment) out += `<span class="sql-comment">${escapeHtml(t)}</span>`;
    else if (str) out += `<span class="sql-string">${escapeHtml(t)}</span>`;
    else if (quoted) out += `<span class="sql-ident">${escapeHtml(t)}</span>`;
    else if (num) out += `<span class="sql-number">${t}</span>`;
    else if (word && KEYWORDS.has(word.toUpperCase())) out += `<span class="sql-keyword">${escapeHtml(t)}</span>`;
    else out += escapeHtml(t);
    last = m.index + t.length;
  }
  return out + escapeHtml(sql.slice(last));
}

// --- Notifications ---
let toastBox;
export function toast(message, kind = 'info', ms = 4000) {
  toastBox ??= document.body.appendChild(h('div.toasts'));
  const el = h(`div.toast.${kind}`, {}, message);
  toastBox.append(el);
  setTimeout(() => el.classList.add('hide'), ms);
  setTimeout(() => el.remove(), ms + 400);
}

// --- Fenêtres modales ---
// buttons : [{ label, kind, value, onclick }] ; renvoie une promesse résolue à la fermeture, avec une
// méthode close(valeur) pour fermer la fenêtre depuis son contenu.
export function modal({ title, body, buttons = [{ label: 'Fermer', value: null }], wide = false }) {
  let closer;
  const promise = new Promise((resolve) => {
    const close = (value) => {
      if (!overlay.isConnected) return;
      overlay.remove();
      document.removeEventListener('keydown', onKey);
      resolve(value);
    };
    const onKey = (e) => {
      if (e.key === 'Escape') close(null);
    };
    const footer = h(
      'div.modal-footer',
      {},
      buttons.map((b) =>
        h(
          `button.btn${b.kind ? '.' + b.kind : ''}`,
          {
            onclick: async () => {
              if (b.onclick) {
                const r = await b.onclick();
                if (r === false) return;
                close(r === undefined ? b.value : r);
              } else close(b.value);
            },
          },
          b.label,
        ),
      ),
    );
    const box = h(`div.modal${wide ? '.wide' : ''}`, {}, h('div.modal-header', {}, h('h3', {}, title), h('button.icon-btn', { onclick: () => close(null), title: 'Fermer' }, '✕')), h('div.modal-body', {}, body), footer);
    const overlay = h('div.overlay', { onmousedown: (e) => e.target === overlay && close(null) }, box);
    document.body.append(overlay);
    document.addEventListener('keydown', onKey);
    box.querySelector('input, select, textarea')?.focus();
    closer = close;
  });
  promise.close = closer;
  return promise;
}

export function confirmDialog(title, message, okLabel = 'Confirmer', kind = 'danger') {
  return modal({
    title,
    body: h('p', {}, message),
    buttons: [
      { label: 'Annuler', value: false },
      { label: okLabel, kind, value: true },
    ],
  });
}

// Tableau simple de résultats (éditeur SQL, listes en lecture seule).
export function resultTable(columns, rows, { onRow, render } = {}) {
  return h(
    'div.table-wrap',
    {},
    h(
      'table.grid',
      {},
      h('thead', {}, h('tr', {}, columns.map((c) => h('th', {}, c.label ?? c)))),
      h(
        'tbody',
        {},
        rows.map((r) =>
          h(
            'tr',
            { class: onRow ? 'clickable' : '', onclick: onRow ? () => onRow(r) : undefined },
            columns.map((c, i) => {
              const v = Array.isArray(r) ? r[i] : r[c.name ?? c];
              return h('td', {}, render ? render(c, v, r) : formatCell(v));
            }),
          ),
        ),
      ),
    ),
  );
}

export function formatCell(v) {
  if (v === null || v === undefined) return h('span.null', {}, 'NULL');
  if (typeof v === 'object' && 'binary' in v) return h('span.null', {}, `<binaire, ${v.binary} octets>`);
  const s = String(v);
  return s.length > 160 ? h('span', { title: s }, s.slice(0, 160) + '…') : s;
}

export async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    toast('Copié dans le presse-papiers', 'success');
  } catch {
    toast('Copie impossible : sélectionner le texte à la main', 'error');
  }
}

export const store = {
  get(key, fallback) {
    try {
      const v = localStorage.getItem(`archforge.${key}`);
      return v === null ? fallback : JSON.parse(v);
    } catch {
      return fallback;
    }
  },
  set(key, value) {
    try {
      localStorage.setItem(`archforge.${key}`, JSON.stringify(value));
    } catch {
      /* stockage indisponible : réglage non retenu */
    }
  },
};
