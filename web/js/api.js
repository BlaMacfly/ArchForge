// Appels à l'API locale d'ArchForge (src/api.rs).

const token = document.querySelector('meta[name="archforge-token"]')?.content ?? '';

async function call(method, path, body) {
  const res = await fetch(`/api${path}`, {
    method,
    headers: { 'Content-Type': 'application/json', 'X-ArchForge-Token': token },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const data = await res.json().catch(() => ({ error: `réponse illisible (${res.status})` }));
  if (!res.ok) throw new Error(data.error ?? `erreur ${res.status}`);
  return data;
}

export const api = {
  defaults: () => call('GET', '/defaults'),
  status: () => call('GET', '/status'),
  connect: (settings) => call('POST', '/connect', settings),
  disconnect: () => call('POST', '/disconnect', {}),
  overview: () => call('GET', '/overview'),
  // Lecture paramétrée : une instruction, des « ? » remplacés par params.
  select: (db, sql, params = []) => call('POST', '/select', { db, sql, params }),
  // Script (plusieurs instructions), dans une transaction par défaut.
  execute: (db, sql, transaction = true) => call('POST', '/execute', { db, sql, transaction }),
  contentList: () => call('GET', '/content'),
  contentRead: (name) => call('GET', `/content/file/${encodeURIComponent(name)}`),
  contentAppend: (body) => call('POST', '/content', body),
  contentDir: (path) => call('POST', '/content/dir', { path }),
};

// Lignes d'un résultat sous forme d'objets { colonne: valeur }.
export async function rows(db, sql, params = []) {
  const r = await api.select(db, sql, params);
  return r.rows.map((row) => Object.fromEntries(r.columns.map((c, i) => [c, row[i]])));
}

export async function one(db, sql, params = []) {
  return (await rows(db, sql, params))[0] ?? null;
}
