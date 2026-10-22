// Routeur et mise en page d'ArchForge.
//   #/connexion           connexion à MariaDB
//   #/                    tableau de bord
//   #/e/<éditeur>         recherche et création
//   #/e/<éditeur>/<id>    édition (?new=1&copy=<id> pour une création, ?tab=<onglet>)
//   #/sql                 éditeur SQL
//   #/contenu             couche de contenu

import { api } from './api.js';
import { h, clear, confirmDialog, toast } from './ui.js';
import { ENTITIES, byRoute } from './defs/index.js';
import { EntityEditor, searchPage, rememberRecent } from './editor.js';
import { connectionPage, dashboardPage, sqlPage, contentPage } from './pages.js';

const sidebar = document.getElementById('sidebar');
const main = document.getElementById('main');
let editor = null;
let lastHash = location.hash;
let status = null;

function renderSidebar(active) {
  if (!status?.connected) {
    sidebar.hidden = true;
    return;
  }
  sidebar.hidden = false;
  const link = (href, icon, label, key) => h('a.nav-link', { href, class: active === key ? 'active' : '' }, h('span.nav-icon', {}, icon), label);
  const groups = new Map();
  for (const e of ENTITIES) groups.set(e.group, [...(groups.get(e.group) ?? []), e]);
  clear(
    sidebar,
    h('a.brand', { href: '#/' }, h('img', { src: 'img/emblem-64.png', alt: '' }), h('span', {}, 'ArchForge')),
    h('nav', {}, link('#/', '🏠', 'Tableau de bord', 'home'), [...groups].map(([group, list]) => [h('div.nav-group', {}, group), list.map((e) => link(`#/e/${e.route}`, e.icon, e.label, e.route))]), h('div.nav-group', {}, 'Outils'), link('#/sql', '⌨️', 'Éditeur SQL', 'sql'), link('#/contenu', '📦', 'Couche de contenu', 'contenu')),
    h(
      'div.sidebar-foot',
      {},
      h('div.muted', {}, `${status.settings.user}@${status.settings.host}`),
      h('div.muted', {}, status.settings.world),
      h('button.btn.small', {
        onclick: async () => {
          if (editor?.dirty && !(await confirmDialog('Déconnexion', 'Des modifications ne sont pas enregistrées. Se déconnecter quand même ?', 'Se déconnecter'))) return;
          editor = null;
          await api.disconnect();
          location.hash = '#/connexion';
        },
      }, 'Déconnexion'),
    ),
  );
}

async function route() {
  if (editor?.dirty && location.hash !== lastHash) {
    const leave = await confirmDialog('Modifications non enregistrées', 'Quitter cette entrée sans enregistrer ses modifications ?', 'Quitter');
    if (!leave) {
      history.replaceState(null, '', lastHash);
      return;
    }
  }
  lastHash = location.hash;
  editor = null;
  const [path, query = ''] = (location.hash || '#/').slice(1).split('?');
  const parts = path.split('/').filter(Boolean).map(decodeURIComponent);
  const params = new URLSearchParams(query);

  try {
    status = await api.status();
  } catch (e) {
    clear(main, h('p.error', {}, e.message));
    return;
  }
  if (!status.connected || parts[0] === 'connexion') {
    renderSidebar(null);
    if (status.connected) {
      location.hash = '#/';
      return;
    }
    await connectionPage(main, () => (location.hash = '#/'));
    return;
  }

  const [page, routeName, id] = parts;
  renderSidebar(page === 'e' ? routeName : page ?? 'home');
  main.scrollTop = 0;
  const entity = page === 'e' ? byRoute[routeName] : null;
  const title = entity ? (id === undefined ? entity.label : `${entity.singular} ${id}`) : { sql: 'Éditeur SQL', contenu: 'Couche de contenu' }[page];
  document.title = title ? `${title} — ArchForge` : 'ArchForge';
  if (!page) return dashboardPage(main, status);
  if (page === 'sql') return sqlPage(main);
  if (page === 'contenu') return contentPage(main, status);
  if (entity) {
    if (id === undefined) return searchPage(main, entity);
    const isNew = params.get('new') === '1';
    editor = new EntityEditor(entity, id, { isNew, copyFrom: params.get('copy') });
    if (!isNew) rememberRecent(entity, id);
    await editor.mount(main);
    return;
  }
  clear(main, h('p.error', {}, 'Page inconnue.'), h('a', { href: '#/' }, 'Retour au tableau de bord'));
}

window.addEventListener('hashchange', route);
window.addEventListener('beforeunload', (e) => {
  if (editor?.dirty) e.preventDefault();
});
window.addEventListener('unhandledrejection', (e) => toast(e.reason?.message ?? String(e.reason), 'error', 8000));
route();
