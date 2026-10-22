// Quêtes (questtemplate.ini) et groupes de PNJ qui les donnent (questgroup.ini).

import { localizedTab } from './common.js';

export const quest = {
  route: 'quest',
  label: 'Quêtes',
  singular: 'Quête',
  icon: '📜',
  group: 'Quêtes',
  db: 'world',
  table: 'quest_template',
  key: { column: 'tid', label: 'TID' },
  ref: 'quest',
  help: 'Le client affiche les quêtes de ses propres fichiers : une quête neuve doit y avoir son TID. Les conditions, objectifs et récompenses lus par le serveur sont marqués « serveur ».',
  create: { nextId: 'SELECT COALESCE(MAX(tid), 0) + 1 AS n FROM quest_template' },
  search: {
    from: 'quest_template',
    columns: [
      { expr: 'tid', name: 'tid', label: 'TID' },
      { expr: 'name', name: 'name', label: 'Nom' },
      { expr: "JSON_VALUE(data, '$.Need_Level_Min')", name: 'lvl', label: 'Niv. min.' },
      { expr: "JSON_VALUE(data, '$.GiveNpcName')", name: 'giver', label: 'Donnée par' },
      { expr: "JSON_VALUE(data, '$.ScenarioName')", name: 'scenario', label: 'Scénario' },
    ],
    filters: [
      { name: 'tid', label: 'TID', expr: 'tid', op: '=' },
      { name: 'name', label: 'Nom', expr: 'name', op: 'like' },
      { name: 'scenario', label: 'Scénario', expr: "JSON_VALUE(data, '$.ScenarioName')", op: 'like' },
    ],
    order: 'tid',
  },
  tabs: [
    {
      id: 'quest',
      label: 'Quête',
      kind: 'single',
      main: true,
      table: 'quest_template',
      key: (id) => ({ tid: Number(id) }),
      json: ['data'],
      defaults: () => ({ name: 'Nouvelle quête', data: { QuestName: 'Nouvelle quête', Category: '1', Need_Level_Min: '1' } }),
      fields: [
        { name: 'tid', label: 'TID', type: 'int', readonly: true },
        { name: 'name', label: 'Nom', type: 'text', server: true },
        {
          name: 'data',
          label: 'Colonnes de questtemplate.ini',
          type: 'json',
          sections: [
            {
              title: 'Conditions',
              keys: [
                { key: 'Need_Level_Min', label: 'Niveau min.', type: 'int', server: true },
                { key: 'Need_Level_Max', label: 'Niveau max.', type: 'int', server: true },
                { key: 'Need_Race', label: 'Race', type: 'int', server: true, help: '1 humain, 2 orc, 3 elfe de lune, 4 dragonien ; vide : toutes.' },
                { key: 'Need_Class', label: 'Classe', type: 'int', server: true, help: '1 combattant, 2 tireur, 3 scion, 4 mage ; vide : toutes.' },
                { key: 'Need_PreviousQuestTid', label: 'Quête préalable', type: 'int', ref: 'quest', server: true },
                { key: 'Self_Quest', label: 'Quête personnelle', type: 'int', server: true, help: '1 : se termine sans revoir de PNJ.' },
                { key: 'QuestGroupID', label: 'Groupe', type: 'int', ref: 'quest_group', server: true },
                { key: 'Category', label: 'Catégorie', type: 'int' },
              ],
            },
            {
              title: 'Objectifs',
              keys: [
                { key: 'Complete_MonsterA_Tid', label: 'Monstre à tuer', type: 'int', ref: 'creature', server: true },
                { key: 'Complete_MonsterA_Count', label: 'Nombre', type: 'int', server: true },
                { key: 'Complete_ItemTid', label: 'Objet à rapporter', type: 'int', ref: 'item', server: true },
                { key: 'Complete_ItemCount', label: 'Quantité', type: 'int', server: true },
                { key: 'Complete_Level', label: 'Niveau à atteindre', type: 'int', server: true },
                { key: 'DropItemA_TID', label: 'Objet de quête lâché', type: 'int', ref: 'item', server: true, help: 'Objet que lâche le monstre visé tant que la quête est en cours.' },
                { key: 'DropItemA_Count', label: 'Quantité lâchée', type: 'int', server: true },
                { key: 'DropItemA_Property', label: 'Probabilité', type: 'int', server: true },
              ],
            },
            {
              title: 'Récompenses',
              keys: [
                { key: 'Gift_Exp', label: 'Expérience', type: 'int', server: true },
                { key: 'Gift_Money', label: 'Or', type: 'int', server: true },
                { key: 'Gift_ItemA_Tid', label: 'Objet', type: 'int', ref: 'item', server: true },
                { key: 'Gift_ItemA_Count', label: 'Quantité', type: 'int', server: true },
              ],
            },
            {
              title: 'Textes (client)',
              keys: [
                { key: 'QuestName', label: 'Nom' },
                { key: 'GiveNpcName', label: 'PNJ donneur' },
                { key: 'ConfirmNpcName', label: 'PNJ de fin' },
                { key: 'Dialogue_Start', label: 'Dialogue de départ' },
                { key: 'Dialogue_Progress', label: 'Dialogue en cours' },
                { key: 'Dialogue_Finish', label: 'Dialogue de fin' },
                { key: 'Complete_Summary1', label: 'Résumé 1' },
                { key: 'Complete_Summary2', label: 'Résumé 2' },
              ],
            },
          ],
        },
      ],
    },
    {
      id: 'groups',
      label: 'Groupes de PNJ',
      kind: 'list',
      help: 'Groupes (questgroup.ini) dont les PNJ proposent ou terminent la quête.',
      empty: "Aucun groupe ne propose cette quête : aucun PNJ ne la donne.",
      query: (id) => [
        "SELECT id, name, IF(JSON_CONTAINS(give, ?), 'oui', '') AS donne, IF(JSON_CONTAINS(confirm, ?), 'oui', '') AS termine FROM quest_group WHERE JSON_CONTAINS(give, ?) OR JSON_CONTAINS(confirm, ?) ORDER BY id",
        Array(4).fill(String(Number(id))),
      ],
      columns: [
        { name: 'id', label: 'Groupe', ref: 'quest_group' },
        { name: 'name', label: 'Nom' },
        { name: 'donne', label: 'Propose' },
        { name: 'termine', label: 'Termine' },
      ],
    },
    localizedTab(['quest_name', 'quest_desc']),
  ],
};

export const questGroup = {
  route: 'quest-group',
  label: 'Groupes de quêtes',
  singular: 'Groupe de quêtes',
  icon: '📚',
  group: 'Quêtes',
  db: 'world',
  table: 'quest_group',
  key: { column: 'id', label: 'Identifiant' },
  ref: 'quest_group',
  help: 'Un PNJ propose et termine les quêtes du groupe désigné par le QuestGroupID de sa fonction 27.',
  create: { nextId: 'SELECT COALESCE(MAX(id), 0) + 1 AS n FROM quest_group' },
  search: {
    from: 'quest_group',
    columns: [
      { expr: 'id', name: 'id', label: 'ID' },
      { expr: 'name', name: 'name', label: 'Nom' },
      { expr: 'JSON_LENGTH(give)', name: 'give', label: 'Proposées' },
      { expr: 'JSON_LENGTH(confirm)', name: 'confirm', label: 'Terminées' },
    ],
    filters: [
      { name: 'id', label: 'ID', expr: 'id', op: '=' },
      { name: 'name', label: 'Nom', expr: 'name', op: 'like' },
    ],
    order: 'id',
  },
  tabs: [
    {
      id: 'group',
      label: 'Groupe',
      kind: 'single',
      main: true,
      table: 'quest_group',
      key: (id) => ({ id: Number(id) }),
      json: ['give', 'confirm'],
      defaults: () => ({ name: 'Nouveau groupe', give: [], confirm: [] }),
      fields: [
        { name: 'id', label: 'ID', type: 'int', readonly: true },
        { name: 'name', label: 'Nom', type: 'text' },
        { name: 'give', label: 'Quêtes proposées', type: 'list', ref: 'quest', server: true },
        { name: 'confirm', label: 'Quêtes terminées ici', type: 'list', ref: 'quest', server: true },
      ],
    },
    {
      id: 'npcs',
      label: 'PNJ',
      kind: 'list',
      empty: "Aucun PNJ n'utilise ce groupe.",
      query: (id) => [
        `SELECT id, name, tid FROM npc_spawn WHERE raw REGEXP CONCAT('"QuestGroupID", ?"', ?, '"[]]') ORDER BY id`,
        [String(id)],
      ],
      columns: [
        { name: 'id', label: 'PNJ', ref: 'npc' },
        { name: 'name', label: 'Nom' },
        { name: 'tid', label: 'Modèle', ref: 'creature' },
      ],
    },
  ],
};
