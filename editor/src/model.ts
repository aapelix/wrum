import { TYPES, DEFAULT_ANCHOR, type TypeDef, type Column } from "./config";

export interface Item {
  id: number;
  type: string;
  v: Record<string, number>;
  s: Record<string, string>;
  /** Absolute world-space vertices for `geometry: "line"` items. */
  points?: { x: number; y: number }[];
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

export const GEOM = new Set(["x", "y", "w", "h", "r"]);
export const D2R = Math.PI / 180;

let nextId = 1;
export const newId = () => nextId++;
export const bumpId = (n: number) => {
  if (n >= nextId) nextId = n + 1;
};

export function typeOf(key: string): TypeDef {
  const t = TYPES.find((t) => t.key === key);
  if (!t) throw new Error("Unknown type " + key);
  return t;
}
export const hasCol = (t: TypeDef, name: string) =>
  t.columns.some((c) => c.name === name);
export const isRect = (t: TypeDef) =>
  t.geometry ? t.geometry === "rect" : hasCol(t, "w") && hasCol(t, "h");
export const isLine = (t: TypeDef) => t.geometry === "line";
export const canRotate = (t: TypeDef) => isRect(t) && hasCol(t, "r");
export const valOf = (it: Item, c: Column) =>
  c.options ? (it.s[c.name] ?? c.options[0]) : (it.v[c.name] ?? 0);
const anchorOf = (t: TypeDef) => t.anchor ?? DEFAULT_ANCHOR;

export function makeItem(
  t: TypeDef,
  sticky?: Record<string, number | string>,
): Item {
  const v: Record<string, number> = {};
  const s: Record<string, string> = {};
  for (const c of t.columns) {
    if (c.options) {
      const st = sticky?.[c.name];
      s[c.name] =
        typeof st === "string" && c.options.includes(st) ? st : c.options[0];
    } else {
      v[c.name] = GEOM.has(c.name)
        ? c.default
        : ((sticky?.[c.name] as number | undefined) ?? c.default);
    }
  }
  if (hasCol(t, "w")) v.w = t.defaultSize?.[0] ?? 32;
  if (hasCol(t, "h")) v.h = t.defaultSize?.[1] ?? 32;
  const item: Item = { id: newId(), type: t.key, v, s };
  if (isLine(t)) item.points = [];
  return item;
}

/** Center-based geometry of an item (points have w = h = 0, lines use their bbox). */
export function geo(it: Item): Geo {
  const t = typeOf(it.type);
  if (isLine(t)) {
    const pts = it.points ?? [];
    if (!pts.length) return { cx: 0, cy: 0, w: 0, h: 0, r: 0 };
    let x0 = Infinity;
    let y0 = Infinity;
    let x1 = -Infinity;
    let y1 = -Infinity;
    for (const p of pts) {
      x0 = Math.min(x0, p.x);
      y0 = Math.min(y0, p.y);
      x1 = Math.max(x1, p.x);
      y1 = Math.max(y1, p.y);
    }
    return { cx: (x0 + x1) / 2, cy: (y0 + y1) / 2, w: x1 - x0, h: y1 - y0, r: 0 };
  }
  const [ax, ay] = anchorOf(t);
  const w = it.v.w ?? 0;
  const h = it.v.h ?? 0;
  const r = it.v.r ?? 0;
  const c = Math.cos(r * D2R);
  const s = Math.sin(r * D2R);
  const ox = (0.5 - ax) * w;
  const oy = (0.5 - ay) * h;
  return {
    cx: it.v.x + c * ox - s * oy,
    cy: it.v.y + s * ox + c * oy,
    w,
    h,
    r,
  };
}

/** Set an item from center-based geometry (writes x, y, w, h, r as far as the type has them). */
export function setFromCenter(
  it: Item,
  cx: number,
  cy: number,
  w: number,
  h: number,
  r: number,
) {
  const t = typeOf(it.type);
  if (isLine(t)) return;
  const [ax, ay] = anchorOf(t);
  if (hasCol(t, "w")) it.v.w = w;
  if (hasCol(t, "h")) it.v.h = h;
  if (hasCol(t, "r")) it.v.r = r;
  const rr = hasCol(t, "r") ? r : 0;
  const c = Math.cos(rr * D2R);
  const s = Math.sin(rr * D2R);
  const ox = (0.5 - ax) * w;
  const oy = (0.5 - ay) * h;
  it.v.x = cx - (c * ox - s * oy);
  it.v.y = cy - (s * ox + c * oy);
}

const round2 = (n: number | undefined) => {
  const r = Math.round((n ?? 0) * 100) / 100;
  return Number.isFinite(r) && r !== 0 ? r : 0;
};
const fmt = (n: number | undefined) => String(round2(n));

/** One array per line, matching the hand-written format.
 *  Line rows start with their `[[x, y], ...]` point list, then the columns. */
export function serialize(doc: Doc): string {
  const parts: string[] = [];
  for (const t of TYPES) {
    const rows = doc.items
      .filter((i) => i.type === t.key)
      .map((i) => {
        const cells = t.columns.map((c) =>
          c.options
            ? JSON.stringify(i.s[c.name] ?? c.options[0])
            : fmt(i.v[c.name]),
        );
        if (isLine(t))
          cells.unshift(
            JSON.stringify((i.points ?? []).map((p) => [round2(p.x), round2(p.y)])),
          );
        return "    [" + cells.join(", ") + "]";
      });
    parts.push(
      rows.length
        ? `  "${t.key}": [\n${rows.join(",\n")}\n  ]`
        : `  "${t.key}": []`,
    );
  }
  for (const [k, val] of Object.entries(doc.extra))
    parts.push(`  ${JSON.stringify(k)}: ${JSON.stringify(val)}`);
  return "{\n" + parts.join(",\n") + "\n}\n";
}

const asPoint = (v: unknown): { x: number; y: number } | null => {
  if (Array.isArray(v)) {
    const x = Number(v[0]);
    const y = Number(v[1]);
    if (v.length >= 2 && Number.isFinite(x) && Number.isFinite(y))
      return { x, y };
  } else if (v && typeof v === "object") {
    const o = v as Record<string, unknown>;
    const x = Number(o.x);
    const y = Number(o.y);
    if (Number.isFinite(x) && Number.isFinite(y)) return { x, y };
  }
  return null;
};

/** Make sure a line item has at least two finite points. */
export function ensurePoints(it: Item): void {
  const t = typeOf(it.type);
  if (!isLine(t)) return;
  const pts = (it.points ?? []).filter(
    (p) => Number.isFinite(p.x) && Number.isFinite(p.y),
  );
  if (pts.length >= 2) {
    it.points = pts;
    return;
  }
  if (pts.length === 1) {
    it.points = [pts[0], { x: pts[0].x + 32, y: pts[0].y }];
    return;
  }
  const x = it.v.x ?? 0;
  const y = it.v.y ?? 0;
  it.points = [
    { x, y },
    { x: x + 32, y },
  ];
}

export function parseDoc(text: string): Doc {
  const data: unknown = JSON.parse(text);
  if (!data || typeof data !== "object" || Array.isArray(data))
    throw new Error("Expected a JSON object at top level");
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
      let offset = 0;
      if (isLine(t)) {
        const raw = row[0];
        if (Array.isArray(raw)) {
          const pts = raw
            .map(asPoint)
            .filter((p): p is { x: number; y: number } => p !== null);
          if (pts.length < 2) continue;
          it.points = pts;
          offset = 1;
        } else if (
          typeof row[0] === "number" &&
          typeof row[1] === "number" &&
          typeof row[2] === "number" &&
          typeof row[3] === "string"
        ) {
          // Legacy one-segment rows: [x, y, rotation, kind].
          const kind = row[3];
          const len = kind === "fence_end" ? 4 : 17;
          const a = Number(row[2]) * D2R;
          const dx = (Math.cos(a) * len) / 2;
          const dy = (Math.sin(a) * len) / 2;
          it.points = [
            { x: Number(row[0]) - dx, y: Number(row[1]) - dy },
            { x: Number(row[0]) + dx, y: Number(row[1]) + dy },
          ];
          for (const c of t.columns)
            if (c.options?.includes(kind)) it.s[c.name] = kind;
          items.push(it);
          continue;
        } else {
          continue;
        }
      }
      t.columns.forEach((c, i) => {
        const value = row[i + offset];
        if (c.options) {
          if (typeof value === "string") it.s[c.name] = value;
          return;
        }
        const n = Number(value);
        if (value !== undefined && value !== null && Number.isFinite(n))
          it.v[c.name] = n;
      });
      items.push(it);
    }
  }
  return { items, extra };
}
