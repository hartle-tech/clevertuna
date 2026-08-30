// Push assets/clvx-s-layout.json into the Theme Builder artboard.
//
// The artboard's logic runs in a sandboxed preview and cannot read files, so
// the layout is inlined between markers. This keeps one source of truth: edit
// the JSON, run this, re-seed.
//
// Usage: node sync-layout.mjs

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const layout = JSON.parse(readFileSync(join(here, '../../assets/clvx-s-layout.json'), 'utf8'));

// Only what the drawing needs — the prose notes stay in the JSON.
const slim = {
  unit: layout.unit,
  rows: layout.rows.map((r) => ({
    y: r.y,
    h: r.h,
    keys: r.keys.map((k) => {
      const o = { x: k.x, w: k.w, label: k.label };
      if (k.sub) o.sub = k.sub;
      if (k.size) o.size = k.size;
      if (k.homing) o.homing = 1;
      if (k.led) o.led = 1;
      if (k.space) o.space = 1;
      return o;
    }),
  })),
  zones: layout.zones.map(({ id, name, shape, x, y, w, h }) => ({ id, name, shape, x, y, w, h })),
};

const BEGIN = '    // LAYOUT-BEGIN (generated from assets/clvx-s-layout.json — do not edit by hand)';
const END = '    // LAYOUT-END';

const file = join(here, 'Main.dc.html');
const src = readFileSync(file, 'utf8');
const a = src.indexOf(BEGIN);
const b = src.indexOf(END);
if (a < 0 || b < 0) throw new Error('layout markers missing from Main.dc.html');

const block = `${BEGIN}\n    const LAYOUT = ${JSON.stringify(slim)};\n${END}`;
writeFileSync(file, src.slice(0, a) + block + src.slice(b + END.length));

const keys = slim.rows.reduce((n, r) => n + r.keys.length, 0);
console.error(`Main.dc.html: ${keys} keys, ${slim.rows.length} rows, ${slim.zones.length} zones`);
