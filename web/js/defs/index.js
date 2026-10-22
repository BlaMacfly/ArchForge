// Liste des éditeurs, dans l'ordre du menu.

import creature from './creature.js';
import { npc, shop } from './npc.js';
import item from './item.js';
import { quest, questGroup } from './quest.js';
import skill from './skill.js';
import { teleport, region, dropGroup, language } from './world.js';
import account from './account.js';

export const ENTITIES = [creature, npc, item, shop, dropGroup, quest, questGroup, skill, teleport, region, language, account];

export const byRoute = Object.fromEntries(ENTITIES.map((e) => [e.route, e]));
