import { TYPES, DEFAULT_ANCHOR, type TypeDef } from './config';

export interface Item {
  id: number;
  type: string;
  v: Record<string, number>;
}

export interface Doc {
  items: Item[];
  /** Unknown top-level keys from imported JSON, preserved on export. */
  extra: Record<string, unknown>;
}

export interface Geo {
  cx: number;
  cy: number;
  w: number;
  h: number;
  /** degrees */
  r: number;
}

export const GEOM = new Set(['x', 'y', 'w', 'h', 'r']);
export const D2R = Math.PI / 180;

let nextId = 1;
export const newId = () => nextId++;
export const bumpId = (n: number) => {
  if (n >= nextId) nextId = n + 1;
};

export function typeOf(key: string): TypeDef {
  const t = TYPES.find((t) => t.key === key);
  if (!t) throw new Error('Unknown type ' + key);
  return t;
}
export const hasCol = (t: TypeDef, name: string) => t.columns.some((c) => c.name === name);
export const isRect = (t: TypeDef) => hasCol(t, 'w') && hasCol(t, 'h');
export const canRotate = (t: TypeDef) => isRect(t) && hasCol(t, 'r');
const anchorOf = (t: TypeDef) => t.anchor ?? DEFAULT_ANCHOR;

export function makeItem(t: TypeDef, sticky?: Record<string, number>): Item {
  const v: Record<string, number> = {};
  for (const c of t.columns) v[c.name] = GEOM.has(c.name) ? c.default : (sticky?.[c.name] ?? c.default);
  if (hasCol(t, 'w')) v.w = t.defaultSize?.[0] ?? 32;
  if (hasCol(t, 'h')) v.h = t.defaultSize?.[1] ?? 32;
  return { id: newId(), type: t.key, v };
}

/** Center-based geometry of an item (points have w = h = 0). */
export function geo(it: Item): Geo {
  const t = typeOf(it.type);
  const [ax, ay] = anchorOf(t);
  const w = it.v.w ?? 0;
  const h = it.v.h ?? 0;
  const r = it.v.r ?? 0;
  const c = Math.cos(r * D2R);
  const s = Math.sin(r * D2R);
  const ox = (0.5 - ax) * w;
  const oy = (0.5 - ay) * h;
  return { cx: it.v.x + c * ox - s * oy, cy: it.v.y + s * ox + c * oy, w, h, r };
}

/** Set an item from center-based geometry (writes x, y, w, h, r as far as the type has them). */
export function setFromCenter(it: Item, cx: number, cy: number, w: number, h: number, r: number) {
  const t = typeOf(it.type);
  const [ax, ay] = anchorOf(t);
  if (hasCol(t, 'w')) it.v.w = w;
  if (hasCol(t, 'h')) it.v.h = h;
  if (hasCol(t, 'r')) it.v.r = r;
  const rr = hasCol(t, 'r') ? r : 0;
  const c = Math.cos(rr * D2R);
  const s = Math.sin(rr * D2R);
  const ox = (0.5 - ax) * w;
  const oy = (0.5 - ay) * h;
  it.v.x = cx - (c * ox - s * oy);
  it.v.y = cy - (s * ox + c * oy);
}

const fmt = (n: number | undefined) => {
  const r = Math.round((n ?? 0) * 100) / 100;
  return String(Number.isFinite(r) && r !== 0 ? r : 0);
};

/** One array per line, matching the hand-written format. */
export function serialize(doc: Doc): string {
  const parts: string[] = [];
  for (const t of TYPES) {
    const rows = doc.items
      .filter((i) => i.type === t.key)
      .map((i) => '    [' + t.columns.map((c) => fmt(i.v[c.name])).join(', ') + ']');
    parts.push(rows.length ? `  "${t.key}": [\n${rows.join(',\n')}\n  ]` : `  "${t.key}": []`);
  }
  for (const [k, val] of Object.entries(doc.extra)) parts.push(`  ${JSON.stringify(k)}: ${JSON.stringify(val)}`);
  return '{\n' + parts.join(',\n') + '\n}\n';
}

export function parseDoc(text: string): Doc {
  const data: unknown = JSON.parse(text);
  if (!data || typeof data !== 'object' || Array.isArray(data)) throw new Error('Expected a JSON object at top level');
  const items: Item[] = [];
  const extra: Record<string, unknown> = {};
  for (const [k, val] of Object.entries(data as Record<string, unknown>)) {
    const t = TYPES.find((t) => t.key === k);
    if (!t) {
      extra[k] = val;
      continue;
    }
    if (!Array.isArray(val)) continue;
    for (const row of val) {
      if (!Array.isArray(row)) continue;
      const it = makeItem(t);
      t.columns.forEach((c, i) => {
        const n = Number(row[i]);
        if (row[i] !== undefined && row[i] !== null && Number.isFinite(n)) it.v[c.name] = n;
      });
      items.push(it);
    }
  }
  return { items, extra };
}
