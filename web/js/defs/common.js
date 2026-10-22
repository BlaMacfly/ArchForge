// Valeurs partagées par les éditeurs. Les libellés viennent des données d'ArchCore (modèles de base de
// CharacterDataTable.txt, exemples d'objets d'itemdatatable.txt), pas d'une documentation extérieure.

export const RACE = { 0: 'Aucune / toutes', 1: 'Humain', 2: 'Orc', 3: 'Elfe de lune', 4: 'Dragonien' };

export const CLASS = {
  0: 'Aucune / toutes',
  1: 'Combattant (Chevalier, Berserker, Swashbuckler, Slayer)',
  2: 'Tireur (Archer, Chasseur, Ranger, Orbiter)',
  3: 'Scion',
  4: 'Mage (Mage, Sorcier, Élémentaliste, Summoner)',
};

export const GENDER = { 0: '—', 1: 'Masculin', 2: 'Féminin' };

// Races admises d'un point de téléportation (use_type) : 0 = toutes.
export const RACE_FLAGS = [
  { bit: 1, label: 'Humain' },
  { bit: 2, label: 'Orc' },
  { bit: 4, label: 'Elfe de lune' },
  { bit: 8, label: 'Dragonien' },
];

export const ITEM_TYPE = { 0: 'Équipement', 1: 'Consommable', 2: 'Divers' };

export const ITEM_KIND = {
  0: 'Aucune (cape, non équipable)',
  1: 'Armure',
  2: 'Arme',
  4: 'Bouclier',
  8: 'Anneau',
  16: 'Collier',
  32: 'Monture',
  64: 'Oréade',
};

export const ITEM_PART = {
  0: 'Aucun',
  2: 'Corps',
  3: 'Tête',
  4: 'Bras',
  5: 'Mains',
  6: 'Jambes',
  7: 'Pieds',
  9: 'Main gauche (arc, bouclier)',
  10: 'Main droite',
  11: 'Anneau',
  13: 'Collier',
  14: 'Monture',
  15: 'Arme de monture (lance)',
  16: 'Apparence : corps',
  17: 'Apparence : tête',
  18: 'Apparence : bras',
  19: 'Apparence : mains',
  20: 'Apparence : jambes',
  21: 'Apparence : pieds',
  31: 'Oréade',
};

export const USABLE_TYPES =
  'Consommables (type) : 0 potion, 2 pierre élémentaire, 3 cube, 4 passeport, 5 flèche, 6 carreau, ' +
  '7 potion de transformation, 8 potion de renforcement, 9 pierre de prière, 10 manuel de métamorphose, ' +
  '11 livre de compétence, 12 ragoût, 13 orbe de retour, 14 orbe de résurrection, 15 orbe d’invocation, ' +
  '17 coffret, 18 clé, 34 nourriture de familier, 38 extension d’inventaire.';

export const SAFETY = { 0: 'Sûre', 1: 'Libre', 2: 'Dangereuse' };

export const QUEST_STATUS = { 0: 'En cours', 1: 'Terminée' };

export const GUILD_RANK = { 1: 'Membre', 2: "Demande d'adhésion", 3: 'Demande de départ', 10: 'Chef', 11: 'Second' };

export const ITEM_STATUS = { 0: 'Inventaire', 1: 'Équipé', 2: 'Banque' };

export const LOCALIZED_KINDS = {
  npc_name: 'Nom de PNJ (npc_spawn.id)',
  monster_name: 'Nom de monstre (TID)',
  item_name: "Nom d'objet (TID)",
  item_desc: "Description d'objet (TID)",
  quest_name: 'Nom de quête (TID)',
  quest_desc: 'Description de quête (TID)',
  skill_name: 'Nom de compétence (TID)',
  region_name: 'Nom de région (id)',
  npc_dialog: 'Dialogue de PNJ (réservé)',
};

// Clé stable d'une ligne créée par ArchForge : reconnaissable, unique, assez courte pour la colonne.
export function forgeKey(prefix, max) {
  const suffix = `#af${Date.now().toString(36)}${Math.floor(Math.random() * 1296).toString(36)}`;
  return `${String(prefix).slice(0, max - suffix.length)}${suffix}`;
}

// Localisation d'une entrée : textes localized_text de la sorte donnée.
export function localizedTab(kinds, label = 'Traductions') {
  return {
    id: 'i18n',
    label,
    kind: 'multi',
    table: 'localized_text',
    parent: (id) => ({ kind: kinds.length === 1 ? kinds[0] : kinds, ref_id: Number(id) }),
    order: 'kind, lang',
    rowKey: ['kind', 'ref_id', 'lang'],
    help: "Textes servis aux clients dans leur langue (repli : langue de base, puis anglais, puis texte d'origine). Les lignes « manual » survivent à `archcore i18n db`.",
    newRow: (id) => ({ kind: kinds[0], ref_id: Number(id), lang: 'fr', text: '', source: 'manual' }),
    columns: [
      { name: 'kind', label: 'Sorte', type: 'enum', textEnum: true, enum: Object.fromEntries(kinds.map((k) => [k, LOCALIZED_KINDS[k]])) },
      { name: 'lang', label: 'Langue', type: 'text', ref: 'language' },
      { name: 'text', label: 'Texte', type: 'text' },
      { name: 'source', label: 'Origine', type: 'enum', textEnum: true, enum: { manual: 'manual (saisi)', import: 'import (i18n db)' } },
    ],
  };
}
