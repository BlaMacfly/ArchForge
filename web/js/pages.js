// Pages hors éditeurs : connexion, tableau de bord, éditeur SQL, couche de contenu.

import { api, rows } from './api.js';
import { h, clear, toast, highlightSql, resultTable, store, copyText } from './ui.js';
import { ENTITIES } from './defs/index.js';

const DB_ALIASES = { world: 'Monde', auth: 'Comptes', characters: 'Personnages', logs: 'Journaux' };

// ------------------------------------------------------------------ connexion

export async function connectionPage(container, onConnected) {
  const defaults = await api.defaults().catch(() => ({ settings: {} }));
  const recent = store.get('connections', []);
  const base = { ...defaults.settings, ...(recent[0] ?? {}) };
  const fields = [
    ['host', 'Hôte'],
    ['port', 'Port'],
    ['user', 'Utilisateur'],
    ['password', 'Mot de passe'],
    ['world', 'Base monde'],
    ['auth', 'Base des comptes'],
    ['characters', 'Base des personnages'],
    ['logs', 'Base des journaux'],
  ];
  const inputs = {};
  const remember = h('input', { type: 'checkbox', checked: !!recent[0]?.password });
  const fill = (s) => {
    for (const [k] of fields) inputs[k].value = s[k] ?? '';
    remember.checked = !!s.password;
  };
  const grid = h(
    'div.form-grid',
    {},
    fields.map(([k, label]) => {
      inputs[k] = h('input.input', { type: k === 'password' ? 'password' : 'text', value: base[k] ?? '', autocomplete: k === 'password' ? 'current-password' : 'off' });
      return h('div.field', {}, h('label.field-label', {}, label), inputs[k]);
    }),
  );
  const error = h('p.error');
  const connect = async () => {
    const settings = Object.fromEntries(fields.map(([k]) => [k, inputs[k].value.trim()]));
    settings.port = Number(settings.port) || 3306;
    error.textContent = '';
    try {
      const r = await api.connect(settings);
      const saved = { ...settings, password: remember.checked ? settings.password : '' };
      const others = recent.filter((c) => !(c.host === saved.host && c.port === saved.port && c.user === saved.user && c.world === saved.world));
      store.set('connections', [saved, ...others].slice(0, 6));
      toast(`Connecté à MariaDB ${r.version}`, 'success');
      onConnected();
    } catch (e) {
      error.textContent = e.message;
    }
  };
  grid.addEventListener('keydown', (e) => e.key === 'Enter' && connect());
  clear(
    container,
    h(
      'div.connect-page',
      {},
      h('img.connect-logo', { src: 'img/logo.png', alt: 'ArchForge' }),
      h('p.muted.center', {}, "Éditeur de la base de données d'ArchCore"),
      h(
        'section.card',
        {},
        h('h3', {}, 'Connexion à MariaDB'),
        defaults.archcore ? h('p.muted', {}, `Valeurs lues dans config/world.conf de ${defaults.archcore}`) : h('p.muted', {}, 'Lancer ArchForge avec --archcore <dépôt ArchCore> pour reprendre sa configuration et activer la couche de contenu.'),
        grid,
        h('label.check', {}, remember, ' Mémoriser le mot de passe sur ce poste'),
        error,
        h('div.toolbar', {}, h('button.btn.primary', { onclick: connect }, 'Se connecter')),
      ),
      recent.length
        ? h(
            'section.card',
            {},
            h('h3', {}, 'Connexions récentes'),
            h('div.chips', {}, recent.map((c) => h('button.chip', { onclick: () => fill(c) }, `${c.user}@${c.host}:${c.port} — ${c.world}`))),
          )
        : null,
    ),
  );
}

// ------------------------------------------------------------------ tableau de bord

export async function dashboardPage(container, status) {
  clear(container, h('p.muted', {}, 'Chargement…'));
  let overview;
  try {
    overview = await api.overview();
  } catch (e) {
    clear(container, h('p.error', {}, e.message));
    return;
  }
  const world = overview.world;
  const count = (table) => world?.tables.find((t) => t.name === table)?.rows ?? '—';
  let imports = [];
  let contentFiles = null;
  try {
    imports = await rows('world', 'SELECT source, files, archcore_version, CAST(started_at AS CHAR) AS started, CAST(finished_at AS CHAR) AS finished FROM import_run ORDER BY id DESC LIMIT 6');
  } catch {
    /* base d'avant la migration import_run */
  }
  try {
    contentFiles = await api.contentList();
  } catch {
    contentFiles = null;
  }

  const tiles = ENTITIES.filter((e) => e.db === 'world').map((e) =>
    h('a.tile', { href: `#/e/${e.route}` }, h('span.tile-icon', {}, e.icon), h('span.tile-label', {}, e.label), h('span.tile-count', {}, String(count(e.table)))),
  );

  clear(
    container,
    h('header.page-head', {}, h('div', {}, h('h2', {}, 'Tableau de bord'), h('p.muted', {}, `MariaDB ${status.version} — ${status.settings.user}@${status.settings.host}:${status.settings.port}`))),
    h(
      'section.card.notice',
      {},
      h('h3', {}, 'À savoir'),
      h(
        'ul',
        {},
        h('li', {}, "ArchCore charge le monde au démarrage : redémarrer le serveur pour qu'il voie les modifications."),
        h('li', {}, "`archcore import` régénère la base monde depuis les fichiers du client : ce qui doit durer va dans la couche de contenu (bouton « Ajouter au contenu »), rejouée après chaque import."),
        h('li', {}, 'Chaque requête est affichée avant de partir et passe dans une transaction : tout ou rien.'),
      ),
    ),
    h('div.tiles', {}, tiles),
    h(
      'div.columns',
      {},
      h(
        'section.card',
        {},
        h('h3', {}, 'Bases'),
        resultTable(
          [{ name: 'alias', label: 'Rôle' }, { name: 'schema', label: 'Base' }, { name: 'tables', label: 'Tables' }, { name: 'migration', label: 'Dernière migration' }],
          Object.entries(overview).map(([alias, b]) => ({
            alias: DB_ALIASES[alias] ?? alias,
            schema: b.schema,
            tables: b.exists ? b.tables.length : 'absente',
            migration: b.migrations.at(-1)?.id ?? '—',
          })),
        ),
      ),
      h(
        'section.card',
        {},
        h('h3', {}, 'Couche de contenu'),
        status.content ? h('p.muted', {}, status.content) : h('p.muted', {}, 'Aucun dossier : voir la page Couche de contenu.'),
        contentFiles ? h('p', {}, `${contentFiles.length} fichier(s)`, ' — ', h('a', { href: '#/contenu' }, 'ouvrir')) : null,
        imports.length ? h('h4', {}, 'Derniers imports') : null,
        imports.length ? resultTable([{ name: 'source', label: 'Source' }, { name: 'files', label: 'Fichiers' }, { name: 'archcore_version', label: 'Version' }, { name: 'finished', label: 'Fin' }], imports) : null,
      ),
    ),
  );
}

// ------------------------------------------------------------------ éditeur SQL

export function sqlPage(container) {
  const db = h('select.input', {}, Object.entries(DB_ALIASES).map(([k, v]) => h('option', { value: k, selected: k === store.get('sql.db', 'world') }, v)));
  const editor = h('textarea.input.code', { rows: 12, spellcheck: false, value: store.get('sql.text', 'SELECT tid, name, npc_price FROM item_data WHERE name LIKE \'%Bow%\' LIMIT 20;') });
  const results = h('div.results');
  const transaction = h('input', { type: 'checkbox', checked: true });
  const run = async () => {
    store.set('sql.text', editor.value);
    store.set('sql.db', db.value);
    clear(results, h('p.muted', {}, 'Exécution…'));
    const started = performance.now();
    try {
      const sets = await api.execute(db.value, editor.value, transaction.checked);
      const ms = Math.round(performance.now() - started);
      clear(
        results,
        h('p.muted', {}, `${sets.length} résultat(s) en ${ms} ms`),
        sets.map((s) =>
          s.columns.length
            ? h('div', {}, resultTable(s.columns, s.rows), s.truncated ? h('p.warning', {}, 'Résultat tronqué à 5 000 lignes.') : null)
            : h('p', {}, `${s.affected} ligne(s) touchée(s)`),
        ),
      );
    } catch (e) {
      clear(results, h('p.error', {}, e.message));
    }
  };
  editor.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      run();
    }
  });
  clear(
    container,
    h('header.page-head', {}, h('div', {}, h('h2', {}, 'Éditeur SQL'), h('p.muted', {}, 'Requêtes libres sur une base d’ArchCore. Ctrl+Entrée pour exécuter.'))),
    h(
      'section.card',
      {},
      h('div.toolbar', {}, h('label.field-label', {}, 'Base'), db, h('label.check', {}, transaction, ' Dans une transaction (tout ou rien)'), h('div.spacer'), h('button.btn', { onclick: () => copyText(editor.value) }, 'Copier'), h('button.btn.primary', { onclick: run }, 'Exécuter')),
      editor,
    ),
    h('section.card', {}, results),
  );
}

// ------------------------------------------------------------------ couche de contenu

export async function contentPage(container, status) {
  const viewer = h('pre.sql.tall');
  const list = h('div.file-list');
  const dirInput = h('input.input', { value: status.content ?? '', placeholder: 'C:\\…\\ArchCore\\content' });
  const load = async () => {
    try {
      const files = await api.contentList();
      clear(
        list,
        files.length
          ? files.map((f) =>
              h(
                'button.file',
                {
                  onclick: async (e) => {
                    list.querySelectorAll('.file').forEach((b) => b.classList.remove('active'));
                    e.currentTarget.classList.add('active');
                    const r = await api.contentRead(f.name);
                    viewer.innerHTML = highlightSql(r.text);
                  },
                },
                h('span', {}, f.name),
                h('span.muted', {}, `${f.size} o`),
              ),
            )
          : h('p.muted', {}, 'Aucun fichier pour le moment.'),
      );
    } catch (e) {
      clear(list, h('p.error', {}, e.message));
    }
  };
  clear(
    container,
    h(
      'header.page-head',
      {},
      h('div', {}, h('h2', {}, 'Couche de contenu'), h('p.muted', {}, "Fichiers <AAAAMMJJHHMM>_<sujet>.sql du dossier content/ d'ArchCore : rejoués par `archcore import` (à la fin) et `archcore content`, chacun dans une transaction, sur la base monde.")),
    ),
    h(
      'section.card',
      {},
      h('h3', {}, 'Dossier'),
      h(
        'div.toolbar',
        {},
        dirInput,
        h('button.btn', {
          onclick: async () => {
            try {
              await api.contentDir(dirInput.value);
              toast('Dossier de contenu choisi', 'success');
              load();
            } catch (e) {
              toast(e.message, 'error', 6000);
            }
          },
        }, 'Utiliser ce dossier'),
      ),
      h('p.muted', {}, 'Règles : données seulement (pas de CREATE, ALTER, DROP…), requêtes rejouables, aucune valeur copiée des fichiers du client. Appliquer ensuite avec `archcore content`.'),
    ),
    h('div.columns.content-columns', {}, h('section.card', {}, h('h3', {}, 'Fichiers'), list), h('section.card', {}, viewer)),
  );
  load();
}
