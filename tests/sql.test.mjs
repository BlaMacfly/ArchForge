// Tests du générateur SQL de l'interface : node --test tests/
import { test } from 'node:test';
import assert from 'node:assert/strict';
import {
  lit, ident, where, deepEqual, jsonPatch, updateQuery, insertQuery, insertAutoIdQuery,
  singleRowDiff, singleRowFull, multiRowDiff, multiRowFull, forbiddenStatements,
} from '../web/js/sql.js';

test('littéraux et identifiants', () => {
  assert.equal(lit(null), 'NULL');
  assert.equal(lit(42), '42');
  assert.equal(lit(-1.5), '-1.5');
  assert.equal(lit("l'arc\\court\n"), "'l\\'arc\\\\court\\n'");
  assert.equal(lit({ a: '1' }), `'{"a":"1"}'`);
  assert.equal(ident('rank'), '`rank`');
  assert.equal(ident('a`b'), '`a``b`');
  assert.equal(where({ tid: 5, name: 'x' }), "`tid` = 5 AND `name` = 'x'");
  assert.equal(where({ deleted_at: null }), '`deleted_at` IS NULL');
  assert.throws(() => lit(NaN));
});

test('égalité : un nombre et son texte sont la même valeur', () => {
  assert.ok(deepEqual(5, '5'));
  assert.ok(deepEqual({ a: [1, { b: '2' }] }, { a: [1, { b: 2 }] }));
  assert.ok(!deepEqual({ a: 1 }, { a: 1, b: 2 }));
  assert.ok(!deepEqual([1, 2], { 0: 1, 1: 2 }));
});

test('patch JSON : seules les clés modifiées', () => {
  const before = { 'NPC Price': '99', Stack: '', DUR: '93' };
  const after = { 'NPC Price': '120', DUR: '93', Nouvelle: '1' };
  assert.equal(
    jsonPatch('raw', before, after),
    `JSON_SET(JSON_REMOVE(\`raw\`, '$."Stack"'), '$."NPC Price"', '120', '$."Nouvelle"', '1')`,
  );
  assert.equal(jsonPatch('raw', before, { ...before }), null);
  assert.equal(jsonPatch('raw', null, { a: '1' }), `'{"a":"1"}'`);
});

test('patch JSON : paires de npc.ini modifiées en place', () => {
  const before = [['Name', 'Darryl'], ['TID', '584'], ['Template', '104']];
  const after = [['Name', 'Darryl'], ['TID', '584'], ['Template', '105']];
  assert.equal(jsonPatch('raw', before, after), `JSON_SET(\`raw\`, '$[2][1]', '105')`);
  // Une paire ajoutée change la longueur : la liste est réécrite.
  const longer = [...after, ['Function', '9']];
  assert.equal(jsonPatch('raw', before, longer), lit(longer));
});

test('patch JSON : liste remplacée par une liste', () => {
  assert.equal(jsonPatch('give', [1, 2], [1, 2, 3]), `'[1,2,3]'`);
  assert.equal(
    jsonPatch('data', { give: [1] }, { give: [1, 2] }),
    `JSON_SET(\`data\`, '$."give"', JSON_EXTRACT('[1,2]', '$'))`,
  );
});

test('UPDATE des seules colonnes changées', () => {
  const before = { tid: 1, name: 'Short Bow', stack: 1, raw: { Stack: '' } };
  const after = { tid: 1, name: 'Arc court', stack: 1, raw: { Stack: '5' } };
  assert.equal(
    updateQuery('item_data', { tid: 1 }, before, after, { json: ['raw'] }),
    "UPDATE `item_data` SET `name` = 'Arc court', `raw` = JSON_SET(`raw`, '$.\"Stack\"', '5') WHERE `tid` = 1;",
  );
  assert.equal(updateQuery('item_data', { tid: 1 }, before, { ...before }), '');
});

test('une ligne : création, modification, entrée complète', () => {
  const row = { tid: 9001, name: 'Épée', raw: { a: '1' } };
  assert.equal(
    singleRowDiff('item_data', ['tid'], null, row, { json: ['raw'] }),
    "INSERT INTO `item_data` (`tid`, `name`, `raw`) VALUES\n(9001, 'Épée', '{\"a\":\"1\"}')\nON DUPLICATE KEY UPDATE `name` = VALUES(`name`), `raw` = VALUES(`raw`);",
  );
  assert.equal(
    singleRowFull('item_data', ['tid'], row, { json: ['raw'] }),
    "DELETE FROM `item_data` WHERE `tid` = 9001;\nINSERT INTO `item_data` (`tid`, `name`, `raw`) VALUES\n(9001, 'Épée', '{\"a\":\"1\"}');",
  );
  // La clé modifiée : la ligne d'origine est visée par son ancienne clé.
  assert.equal(
    singleRowDiff('t', ['tid'], { tid: 1, a: 1 }, { tid: 2, a: 1 }),
    'UPDATE `t` SET `tid` = 2 WHERE `tid` = 1;',
  );
});

test('plusieurs lignes : changements visés par la clé stable', () => {
  const rows = [
    { before: { id: 1, stable_key: '15:5018#1', monster_tid: 15, rate: 10 }, after: { id: 1, stable_key: '15:5018#1', monster_tid: 15, rate: 20 } },
    { before: { id: 2, stable_key: '15:1856#1', monster_tid: 15, rate: 500 }, after: { id: 2, stable_key: '15:1856#1', monster_tid: 15, rate: 500 }, deleted: true },
    { before: { id: 3, stable_key: '15:7#1', monster_tid: 15, rate: 1 }, after: { id: 3, stable_key: '15:7#1', monster_tid: 15, rate: 1 } },
    { before: null, after: { id: null, stable_key: '15:42#af1', monster_tid: 15, rate: 100 } },
  ];
  const sql = multiRowDiff('monster_drop', ['stable_key'], rows, { autoId: 'id', columns: ['id', 'stable_key', 'monster_tid', 'rate'] });
  assert.equal(
    sql,
    "UPDATE `monster_drop` SET `rate` = 20 WHERE `stable_key` = '15:5018#1';\n" +
      "DELETE FROM `monster_drop` WHERE `stable_key` = '15:1856#1';\n" +
      'INSERT INTO `monster_drop` (`id`, `stable_key`, `monster_tid`, `rate`)\n' +
      "SELECT COALESCE(MAX(`id`), 0) + 1, '15:42#af1', 15, 100 FROM `monster_drop`\n" +
      'ON DUPLICATE KEY UPDATE `monster_tid` = VALUES(`monster_tid`), `rate` = VALUES(`rate`);',
  );
  const full = multiRowFull('monster_drop', { monster_tid: 15 }, rows, { autoId: 'id', columns: ['id', 'stable_key', 'monster_tid', 'rate'] });
  assert.match(full, /^DELETE FROM `monster_drop` WHERE `monster_tid` = 15;\nINSERT INTO `monster_drop` \(`id`, `stable_key`, `monster_tid`, `rate`\) VALUES\n\(1, '15:5018#1', 15, 20\),\n\(3, '15:7#1', 15, 1\);\nINSERT INTO `monster_drop` \(`id`/);
});

test('plusieurs lignes sans identifiant automatique : insertion rejouable', () => {
  const rows = [{ before: null, after: { template_id: 104, idx: 7, tid: 1, count: 1 } }];
  assert.equal(
    multiRowDiff('npc_trade_list', ['template_id', 'idx'], rows),
    'INSERT INTO `npc_trade_list` (`template_id`, `idx`, `tid`, `count`) VALUES\n(104, 7, 1, 1)\nON DUPLICATE KEY UPDATE `tid` = VALUES(`tid`), `count` = VALUES(`count`);',
  );
  assert.equal(insertQuery('t', []), '');
  assert.match(insertAutoIdQuery('t', { id: null, k: 'a' }, 'id', { keyColumns: ['k'] }), /SELECT COALESCE\(MAX\(`id`\), 0\) \+ 1, 'a' FROM `t`/);
});

test('couche de contenu : instructions de schéma refusées', () => {
  assert.deepEqual(forbiddenStatements("UPDATE t SET a = 'drop table' -- create\n"), []);
  assert.deepEqual(forbiddenStatements('DROP TABLE x; ALTER TABLE y ADD z INT'), ['ALTER', 'DROP']);
});
