import "./style.css";
import { TYPES, type TypeDef } from "./config";
import {
  type Doc,
  type Item,
  type Geo,
  D2R,
  GEOM,
  typeOf,
  isRect,
  canRotate,
  geo,
  setFromCenter,
  serialize,
  parseDoc,
  makeItem,
  newId,
  bumpId,
} from "./model";

// ---------------------------------------------------------------- DOM
const $ = <T extends HTMLElement>(id: string) =>
  document.getElementById(id) as T;
const cv = $<HTMLCanvasElement>("cv");
const ctx = cv.getContext("2d")!;
const stage = $("stage");
const statusEl = $("status");
const toastEl = $("toast");
const typesEl = $("types");
const propsEl = $("props");
const listEl = $("list");
const snapEl = $<HTMLInputElement>("snap");

function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  attrs: Record<string, string> = {},
  ...children: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
  for (const c of children) el.append(c);
  return el;
}

// ---------------------------------------------------------------- state
type Pt = { x: number; y: number };

const doc: Doc = { items: [], extra: {} };
let selected: number | null = null;
let mode: "select" | "draw" = "select";
let drawType = TYPES[0].key;
const visible: Record<string, boolean> = Object.fromEntries(
  TYPES.map((t) => [t.key, true]),
);
const lastVals: Record<string, Record<string, number>> = {};
const view = { s: 1, ox: 40, oy: 40 };
let img: HTMLImageElement | null = null;
let spaceDown = false;
let mouseWorld: Pt = { x: 0, y: 0 };
const undoStack: string[] = [];
const redoStack: string[] = [];

type Drag =
  | { kind: "pan"; sx: number; sy: number; ox: number; oy: number }
  | {
      kind: "move";
      id: number;
      start: Pt;
      x0: number;
      y0: number;
      snap0: string;
      pushed: boolean;
    }
  | {
      kind: "resize";
      id: number;
      hx: number;
      hy: number;
      g0: Geo;
      snap0: string;
      pushed: boolean;
    }
  | { kind: "rotate"; id: number; g0: Geo; snap0: string; pushed: boolean }
  | {
      kind: "create";
      type: string;
      start: Pt;
      cur: Pt;
      sx: number;
      sy: number;
    };
let drag: Drag | null = null;

const getItem = (id: number | null) =>
  id === null ? undefined : doc.items.find((i) => i.id === id);
const selItem = () => getItem(selected);
const snapOn = () => snapEl.checked;
const rnd = (n: number) => Math.round(n);

// ---------------------------------------------------------------- persistence
const LS_KEY = "collider-editor-doc-v1";

function save() {
  try {
    localStorage.setItem(
      LS_KEY,
      JSON.stringify({ items: doc.items, extra: doc.extra }),
    );
  } catch {
    /* ignore */
  }
}
function loadSaved() {
  try {
    const raw = localStorage.getItem(LS_KEY);
    if (!raw) return;
    const d = JSON.parse(raw) as Doc;
    doc.items = (d.items ?? []).filter((i) =>
      TYPES.some((t) => t.key === i.type),
    );
    doc.extra = d.extra ?? {};
    for (const i of doc.items) bumpId(i.id);
  } catch {
    /* ignore */
  }
}

function idb(): Promise<IDBDatabase> {
  return new Promise((res, rej) => {
    const r = indexedDB.open("collider-editor", 1);
    r.onupgradeneeded = () => r.result.createObjectStore("kv");
    r.onsuccess = () => res(r.result);
    r.onerror = () => rej(r.error);
  });
}
async function idbSet(key: string, val: unknown) {
  try {
    const db = await idb();
    await new Promise<void>((res, rej) => {
      const tx = db.transaction("kv", "readwrite");
      tx.objectStore("kv").put(val, key);
      tx.oncomplete = () => res();
      tx.onerror = () => rej(tx.error);
    });
  } catch {
    /* ignore */
  }
}
async function idbGet<T>(key: string): Promise<T | undefined> {
  try {
    const db = await idb();
    return await new Promise<T | undefined>((res, rej) => {
      const r = db.transaction("kv").objectStore("kv").get(key);
      r.onsuccess = () => res(r.result as T | undefined);
      r.onerror = () => rej(r.error);
    });
  } catch {
    return undefined;
  }
}

// ---------------------------------------------------------------- undo
const snapshot = () => JSON.stringify({ items: doc.items, extra: doc.extra });
function pushUndo(s = snapshot()) {
  undoStack.push(s);
  if (undoStack.length > 200) undoStack.shift();
  redoStack.length = 0;
}
function restore(s: string) {
  const d = JSON.parse(s) as Doc;
  doc.items = d.items;
  doc.extra = d.extra;
  if (selected !== null && !getItem(selected)) selected = null;
  save();
  renderAll();
}
function undo() {
  const s = undoStack.pop();
  if (s === undefined) return;
  redoStack.push(snapshot());
  restore(s);
}
function redo() {
  const s = redoStack.pop();
  if (s === undefined) return;
  undoStack.push(snapshot());
  restore(s);
}
/** Discrete edit: snapshot, run, save, re-render. */
function mutate(fn: () => void) {
  pushUndo();
  fn();
  save();
  renderAll();
}

// ---------------------------------------------------------------- geometry helpers
const cs = (deg: number): [number, number] => [
  Math.cos(deg * D2R),
  Math.sin(deg * D2R),
];
function localOf(g: Geo, p: Pt): Pt {
  const [c, s] = cs(g.r);
  const dx = p.x - g.cx;
  const dy = p.y - g.cy;
  return { x: c * dx + s * dy, y: -s * dx + c * dy };
}
function worldOf(g: Geo, lx: number, ly: number): Pt {
  const [c, s] = cs(g.r);
  return { x: g.cx + c * lx - s * ly, y: g.cy + s * lx + c * ly };
}

interface Handle {
  kind: "rotate" | "resize";
  hx: number;
  hy: number;
  pos: Pt;
}
const HANDLE_DIRS: [number, number][] = [
  [-1, -1],
  [1, -1],
  [1, 1],
  [-1, 1],
  [0, -1],
  [1, 0],
  [0, 1],
  [-1, 0],
];
function handlesOf(it: Item): Handle[] {
  const t = typeOf(it.type);
  if (!isRect(t)) return [];
  const g = geo(it);
  const s = view.s;
  const out: Handle[] = [];
  if (canRotate(t))
    out.push({
      kind: "rotate",
      hx: 0,
      hy: -1,
      pos: worldOf(g, 0, -g.h / 2 - 28 / s),
    });
  for (const [hx, hy] of HANDLE_DIRS) {
    if (hx === 0 && g.w * s < 30) continue;
    if (hy === 0 && g.h * s < 30) continue;
    out.push({
      kind: "resize",
      hx,
      hy,
      pos: worldOf(g, (hx * g.w) / 2, (hy * g.h) / 2),
    });
  }
  return out;
}
function hitHandle(it: Item, p: Pt): Handle | null {
  const r = 8 / view.s;
  // rotate first, then corners, then edges (array order already matches)
  for (const hd of handlesOf(it))
    if (Math.hypot(hd.pos.x - p.x, hd.pos.y - p.y) <= r) return hd;
  return null;
}
/** All items under the point, top-most first. */
function hitAll(p: Pt): Item[] {
  const out: Item[] = [];
  for (let i = doc.items.length - 1; i >= 0; i--) {
    const it = doc.items[i];
    if (!visible[it.type]) continue;
    const t = typeOf(it.type);
    const g = geo(it);
    if (!isRect(t)) {
      if (Math.hypot(p.x - g.cx, p.y - g.cy) <= 9 / view.s) out.push(it);
    } else {
      const m = localOf(g, p);
      const tol = 3 / view.s;
      if (Math.abs(m.x) <= g.w / 2 + tol && Math.abs(m.y) <= g.h / 2 + tol)
        out.push(it);
    }
  }
  return out;
}

// ---------------------------------------------------------------- view
function toWorld(e: { clientX: number; clientY: number }): Pt {
  const r = cv.getBoundingClientRect();
  return {
    x: (e.clientX - r.left - view.ox) / view.s,
    y: (e.clientY - r.top - view.oy) / view.s,
  };
}
function fitView() {
  const cw = stage.clientWidth;
  const ch = stage.clientHeight;
  let x0 = 0,
    y0 = 0,
    x1 = 1000,
    y1 = 800;
  if (img) {
    x1 = img.width;
    y1 = img.height;
  } else if (doc.items.length) {
    x0 = Infinity;
    y0 = Infinity;
    x1 = -Infinity;
    y1 = -Infinity;
    for (const it of doc.items) {
      const g = geo(it);
      const rad = Math.max(g.w, g.h) / 2 + 20;
      x0 = Math.min(x0, g.cx - rad);
      y0 = Math.min(y0, g.cy - rad);
      x1 = Math.max(x1, g.cx + rad);
      y1 = Math.max(y1, g.cy + rad);
    }
  }
  const w = Math.max(1, x1 - x0);
  const hh = Math.max(1, y1 - y0);
  view.s = Math.min(cw / w, ch / hh) * 0.95;
  view.ox = (cw - w * view.s) / 2 - x0 * view.s;
  view.oy = (ch - hh * view.s) / 2 - y0 * view.s;
  requestDraw();
}
function centerOn(p: Pt) {
  view.ox = stage.clientWidth / 2 - p.x * view.s;
  view.oy = stage.clientHeight / 2 - p.y * view.s;
  requestDraw();
}

// ---------------------------------------------------------------- drawing
let drawQueued = false;
function requestDraw() {
  if (drawQueued) return;
  drawQueued = true;
  requestAnimationFrame(draw);
}

function labelOf(t: TypeDef, it: Item): string {
  return t.columns
    .filter((c) => !GEOM.has(c.name))
    .map((c) => `${c.name}=${it.v[c.name]}`)
    .join(" ");
}

function draw() {
  drawQueued = false;
  const dpr = window.devicePixelRatio || 1;
  const cw = stage.clientWidth;
  const ch = stage.clientHeight;
  if (cv.width !== Math.round(cw * dpr) || cv.height !== Math.round(ch * dpr)) {
    cv.width = Math.round(cw * dpr);
    cv.height = Math.round(ch * dpr);
  }
  const s = view.s;
  ctx.setTransform(1, 0, 0, 1, 0, 0);
  ctx.fillStyle = "#14161a";
  ctx.fillRect(0, 0, cv.width, cv.height);
  ctx.setTransform(dpr * s, 0, 0, dpr * s, dpr * view.ox, dpr * view.oy);
  ctx.imageSmoothingEnabled = false;

  if (img) {
    ctx.drawImage(img, 0, 0);
    ctx.strokeStyle = "rgba(255,255,255,0.25)";
    ctx.lineWidth = 1 / s;
    ctx.strokeRect(0, 0, img.width, img.height);
  }

  // pixel grid when zoomed in
  if (s >= 8) {
    const x0 = Math.floor(-view.ox / s);
    const x1 = Math.ceil((cw - view.ox) / s);
    const y0 = Math.floor(-view.oy / s);
    const y1 = Math.ceil((ch - view.oy) / s);
    ctx.beginPath();
    for (let x = x0; x <= x1; x++) {
      ctx.moveTo(x, y0);
      ctx.lineTo(x, y1);
    }
    for (let y = y0; y <= y1; y++) {
      ctx.moveTo(x0, y);
      ctx.lineTo(x1, y);
    }
    ctx.strokeStyle = "rgba(255,255,255,0.12)";
    ctx.lineWidth = 1 / s;
    ctx.stroke();
  }

  // origin marker
  ctx.strokeStyle = "rgba(255,255,255,0.35)";
  ctx.lineWidth = 1 / s;
  ctx.beginPath();
  ctx.moveTo(-10 / s, 0);
  ctx.lineTo(10 / s, 0);
  ctx.moveTo(0, -10 / s);
  ctx.lineTo(0, 10 / s);
  ctx.stroke();

  for (const it of doc.items) if (visible[it.type]) drawItem(it, s);

  const sel = selItem();
  if (sel && visible[sel.type]) drawSelection(sel, s);

  // creation preview
  if (drag?.kind === "create") {
    const t = typeOf(drag.type);
    ctx.strokeStyle = t.color;
    ctx.lineWidth = 2 / s;
    ctx.setLineDash([6 / s, 4 / s]);
    if (isRect(t)) {
      const a = drag.start;
      const b = drag.cur;
      ctx.strokeRect(
        Math.min(a.x, b.x),
        Math.min(a.y, b.y),
        Math.abs(b.x - a.x),
        Math.abs(b.y - a.y),
      );
    }
    ctx.setLineDash([]);
  }

  updateStatus();
}

function drawItem(it: Item, s: number) {
  const t = typeOf(it.type);
  const g = geo(it);
  ctx.lineWidth = 2 / s;
  ctx.strokeStyle = t.color;
  ctx.fillStyle = t.color + "40";
  if (isRect(t)) {
    ctx.save();
    ctx.translate(g.cx, g.cy);
    ctx.rotate(g.r * D2R);
    ctx.fillRect(-g.w / 2, -g.h / 2, g.w, g.h);
    ctx.strokeRect(-g.w / 2, -g.h / 2, g.w, g.h);
    if (canRotate(t)) {
      // "up" tick shows orientation
      ctx.beginPath();
      ctx.moveTo(0, 0);
      ctx.lineTo(0, -g.h / 2);
      ctx.stroke();
    }
    ctx.restore();
  } else {
    const r = 5 / s;
    ctx.beginPath();
    ctx.arc(g.cx, g.cy, r, 0, Math.PI * 2);
    ctx.fill();
    ctx.stroke();
    ctx.beginPath();
    ctx.moveTo(g.cx - r * 2, g.cy);
    ctx.lineTo(g.cx + r * 2, g.cy);
    ctx.moveTo(g.cx, g.cy - r * 2);
    ctx.lineTo(g.cx, g.cy + r * 2);
    ctx.stroke();

    if ("r" in it.v) {
      const angle = (it.v.r ?? 0) * D2R;
      const len = 18 / s;
      const head = 6 / s;
      const tipX = g.cx + Math.cos(angle) * len;
      const tipY = g.cy + Math.sin(angle) * len;

      ctx.beginPath();
      ctx.moveTo(g.cx, g.cy);
      ctx.lineTo(tipX, tipY);
      ctx.moveTo(
        tipX - Math.cos(angle - Math.PI / 6) * head,
        tipY - Math.sin(angle - Math.PI / 6) * head,
      );
      ctx.lineTo(tipX, tipY);
      ctx.lineTo(
        tipX - Math.cos(angle + Math.PI / 6) * head,
        tipY - Math.sin(angle + Math.PI / 6) * head,
      );
      ctx.stroke();
    }
  }
  // (x, y) pivot dot
  ctx.fillStyle = "#fff";
  ctx.beginPath();
  ctx.arc(it.v.x, it.v.y, 2 / s, 0, Math.PI * 2);
  ctx.fill();

  const label = labelOf(t, it);
  if (label) {
    ctx.font = `${12 / s}px ui-monospace, monospace`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.lineWidth = 3 / s;
    ctx.strokeStyle = "#000";
    const ly = isRect(t) ? g.cy : g.cy - 14 / s;
    ctx.strokeText(label, g.cx, ly);
    ctx.fillStyle = "#fff";
    ctx.fillText(label, g.cx, ly);
  }
}

function drawSelection(it: Item, s: number) {
  const t = typeOf(it.type);
  const g = geo(it);
  ctx.strokeStyle = "#fff";
  ctx.lineWidth = 1.5 / s;
  if (isRect(t)) {
    ctx.save();
    ctx.translate(g.cx, g.cy);
    ctx.rotate(g.r * D2R);
    ctx.setLineDash([5 / s, 3 / s]);
    ctx.strokeRect(-g.w / 2, -g.h / 2, g.w, g.h);
    ctx.setLineDash([]);
    ctx.restore();
  } else {
    ctx.beginPath();
    ctx.arc(g.cx, g.cy, 10 / s, 0, Math.PI * 2);
    ctx.stroke();
  }
  const hs = handlesOf(it);
  const rot = hs.find((x) => x.kind === "rotate");
  if (rot) {
    const top = worldOf(g, 0, -g.h / 2);
    ctx.beginPath();
    ctx.moveTo(top.x, top.y);
    ctx.lineTo(rot.pos.x, rot.pos.y);
    ctx.stroke();
  }
  for (const hd of hs) {
    ctx.fillStyle = "#fff";
    ctx.strokeStyle = "#000";
    ctx.lineWidth = 1 / s;
    if (hd.kind === "rotate") {
      ctx.beginPath();
      ctx.arc(hd.pos.x, hd.pos.y, 5 / s, 0, Math.PI * 2);
      ctx.fill();
      ctx.stroke();
    } else {
      const sz = 8 / s;
      ctx.beginPath();
      ctx.rect(hd.pos.x - sz / 2, hd.pos.y - sz / 2, sz, sz);
      ctx.fill();
      ctx.stroke();
    }
  }
}

function updateStatus() {
  statusEl.textContent = `x ${Math.floor(mouseWorld.x)}  y ${Math.floor(mouseWorld.y)}  |  ${Math.round(view.s * 100)}%  |  ${
    mode === "draw" ? "draw: " + typeOf(drawType).label : "select"
  }  |  ${doc.items.length} items`;
}

// ---------------------------------------------------------------- UI rendering
function setMode(m: "select" | "draw") {
  mode = m;
  renderModes();
  requestDraw();
}
function renderModes() {
  $("mode-select").classList.toggle("on", mode === "select");
  $("mode-draw").classList.toggle("on", mode === "draw");
  renderTypes();
}
function renderTypes() {
  typesEl.replaceChildren();
  TYPES.forEach((t, i) => {
    const cnt = doc.items.filter((x) => x.type === t.key).length;
    const vis = h("input", { type: "checkbox", title: "Visible" });
    vis.checked = visible[t.key];
    vis.addEventListener("click", (e) => e.stopPropagation());
    vis.addEventListener("change", () => {
      visible[t.key] = vis.checked;
      requestDraw();
    });
    const sw = h("span", { class: "sw" });
    sw.style.background = t.color;
    const row = h(
      "div",
      {
        class:
          "type-row" + (mode === "draw" && drawType === t.key ? " active" : ""),
      },
      vis,
      sw,
      h("span", { class: "name" }, t.label),
      h("span", { class: "cnt" }, String(cnt)),
      h("span", { class: "key" }, i < 9 ? String(i + 1) : ""),
    );
    row.addEventListener("click", () => chooseDrawType(t.key));
    typesEl.append(row);
  });
}
function chooseDrawType(key: string) {
  drawType = key;
  setMode("draw");
}

function renderProps() {
  propsEl.replaceChildren();
  const it = selItem();
  if (!it) {
    propsEl.append(h("div", { class: "empty" }, "Nothing selected"));
    return;
  }
  const t = typeOf(it.type);
  const grid = h("div", { class: "grid" });

  const typeSel = h("select");
  for (const tt of TYPES) {
    const o = h("option", { value: tt.key }, tt.label);
    if (tt.key === it.type) o.selected = true;
    typeSel.append(o);
  }
  typeSel.addEventListener("change", () => changeType(it, typeSel.value));
  grid.append(h("label", {}, "type"), typeSel);

  for (const c of t.columns) {
    const inp = h("input", { type: "number", step: String(c.step ?? 1) });
    inp.value = String(Math.round((it.v[c.name] ?? 0) * 100) / 100);
    inp.addEventListener("change", () => {
      const n = Number(inp.value);
      if (!Number.isFinite(n)) return renderProps();
      pushUndo();
      it.v[c.name] = n;
      if (!GEOM.has(c.name)) (lastVals[it.type] ??= {})[c.name] = n;
      save();
      renderAll();
    });
    grid.append(h("label", {}, c.name), inp);
  }
  propsEl.append(grid);

  const del = h("button", {}, "Delete");
  del.addEventListener("click", deleteSelected);
  const dup = h("button", {}, "Duplicate");
  dup.addEventListener("click", duplicateSelected);
  propsEl.append(h("div", { class: "actions" }, dup, del));
}

function changeType(it: Item, key: string) {
  if (key === it.type) return;
  mutate(() => {
    const nt = typeOf(key);
    const fresh = makeItem(nt, lastVals[key]);
    for (const c of nt.columns)
      if (c.name in it.v) fresh.v[c.name] = it.v[c.name];
    // keep position; if the old type was a point and the new one a rect, keep the new default size
    it.type = key;
    it.v = fresh.v;
  });
}

function renderList() {
  listEl.replaceChildren();
  for (const t of TYPES) {
    let n = 0;
    for (const it of doc.items) {
      if (it.type !== t.key) continue;
      n++;
      const dot = h("span", { class: "dot" });
      dot.style.background = t.color;
      const meta = t.columns
        .map((c) => `${Math.round((it.v[c.name] ?? 0) * 10) / 10}`)
        .join(", ");
      const row = h(
        "div",
        { class: "item-row" + (it.id === selected ? " sel" : "") },
        dot,
        h("span", {}, `${t.label} ${n}`),
        h("span", { class: "meta" }, meta),
      );
      row.addEventListener("click", () => {
        selected = it.id;
        const g = geo(it);
        centerOn({ x: g.cx, y: g.cy });
        renderAll();
      });
      listEl.append(row);
    }
  }
  if (!doc.items.length)
    listEl.append(
      h("div", { class: "empty" }, "No items yet — pick a type and draw"),
    );
}

function renderAll() {
  renderTypes();
  renderProps();
  renderList();
  requestDraw();
}

let toastTimer = 0;
function toast(msg: string, err = false) {
  toastEl.textContent = msg;
  toastEl.classList.toggle("err", err);
  toastEl.classList.add("show");
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(
    () => toastEl.classList.remove("show"),
    err ? 6000 : 2200,
  );
}

// ---------------------------------------------------------------- actions
function deleteSelected() {
  const it = selItem();
  if (!it) return;
  mutate(() => {
    doc.items = doc.items.filter((i) => i.id !== it.id);
    selected = null;
  });
}
function duplicateSelected() {
  const it = selItem();
  if (!it) return;
  mutate(() => {
    const copy: Item = { id: newId(), type: it.type, v: { ...it.v } };
    copy.v.x += 10;
    copy.v.y += 10;
    doc.items.push(copy);
    selected = copy.id;
  });
}
function nudge(dx: number, dy: number) {
  const it = selItem();
  if (!it) return;
  mutate(() => {
    it.v.x += dx;
    it.v.y += dy;
  });
}

function importText(text: string, label: string) {
  let parsed: Doc;
  try {
    parsed = parseDoc(text);
  } catch (e) {
    toast(`Import failed (${label}): ${(e as Error).message}`, true);
    return;
  }
  mutate(() => {
    doc.items = parsed.items;
    doc.extra = parsed.extra;
    selected = null;
  });
  const extras = Object.keys(parsed.extra);
  toast(
    `Imported ${parsed.items.length} items` +
      (extras.length ? ` (kept unknown keys: ${extras.join(", ")})` : ""),
  );
}

async function setImage(blob: Blob, persist: boolean) {
  const url = URL.createObjectURL(blob);
  const im = new Image();
  await new Promise<void>((res, rej) => {
    im.onload = () => res();
    im.onerror = () => rej(new Error("Could not decode image"));
    im.src = url;
  });
  img = im;
  if (persist) {
    void idbSet("image", blob);
    fitView();
  }
  requestDraw();
}

async function handleFile(f: File) {
  if (f.type.startsWith("image/")) {
    try {
      await setImage(f, true);
      toast(`Loaded ${f.name} (${img!.width}×${img!.height})`);
    } catch (e) {
      toast((e as Error).message, true);
    }
  } else if (
    f.name.toLowerCase().endsWith(".json") ||
    f.type === "application/json"
  ) {
    importText(await f.text(), f.name);
  } else {
    toast("Unsupported file: " + f.name, true);
  }
}

// ---------------------------------------------------------------- pointer interaction
function makeAt(type: string): Item {
  const t = typeOf(type);
  return makeItem(t, lastVals[type]);
}
const snapP = (p: Pt): Pt => (snapOn() ? { x: rnd(p.x), y: rnd(p.y) } : p);

cv.addEventListener("contextmenu", (e) => e.preventDefault());

cv.addEventListener("pointerdown", (e) => {
  cv.setPointerCapture(e.pointerId);
  const p = toWorld(e);
  if (e.button === 1 || e.button === 2 || (e.button === 0 && spaceDown)) {
    drag = {
      kind: "pan",
      sx: e.clientX,
      sy: e.clientY,
      ox: view.ox,
      oy: view.oy,
    };
    cv.style.cursor = "grabbing";
    return;
  }
  if (e.button !== 0) return;

  if (mode === "draw") {
    const sp = snapP(p);
    drag = {
      kind: "create",
      type: drawType,
      start: sp,
      cur: sp,
      sx: e.clientX,
      sy: e.clientY,
    };
    return;
  }

  const sel = selItem();
  if (sel && visible[sel.type]) {
    const hd = hitHandle(sel, p);
    if (hd) {
      const snap0 = snapshot();
      const g0 = geo(sel);
      drag =
        hd.kind === "rotate"
          ? { kind: "rotate", id: sel.id, g0, snap0, pushed: false }
          : {
              kind: "resize",
              id: sel.id,
              hx: hd.hx,
              hy: hd.hy,
              g0,
              snap0,
              pushed: false,
            };
      return;
    }
  }

  const hits = hitAll(p);
  let pick: Item | undefined = hits[0];
  if (e.altKey && hits.length > 1) {
    const i = hits.findIndex((x) => x.id === selected);
    pick = hits[(i + 1) % hits.length];
  }
  selected = pick?.id ?? null;
  if (pick) {
    drag = {
      kind: "move",
      id: pick.id,
      start: p,
      x0: pick.v.x,
      y0: pick.v.y,
      snap0: snapshot(),
      pushed: false,
    };
  }
  renderAll();
});

cv.addEventListener("pointermove", (e) => {
  const p = toWorld(e);
  mouseWorld = p;

  if (!drag) {
    updateCursor(p);
    requestDraw();
    return;
  }

  if (drag.kind === "pan") {
    view.ox = drag.ox + (e.clientX - drag.sx);
    view.oy = drag.oy + (e.clientY - drag.sy);
    requestDraw();
    return;
  }
  if (drag.kind === "create") {
    drag.cur = snapP(p);
    requestDraw();
    return;
  }

  const it = getItem(drag.id);
  if (!it) return;
  const ensureUndo = () => {
    if (drag && drag.kind !== "pan" && drag.kind !== "create" && !drag.pushed) {
      pushUndo(drag.snap0);
      drag.pushed = true;
    }
  };

  if (drag.kind === "move") {
    ensureUndo();
    let nx = drag.x0 + (p.x - drag.start.x);
    let ny = drag.y0 + (p.y - drag.start.y);
    if (snapOn()) {
      nx = rnd(nx);
      ny = rnd(ny);
    }
    it.v.x = nx;
    it.v.y = ny;
  } else if (drag.kind === "rotate") {
    ensureUndo();
    const g0 = drag.g0;
    let r = Math.atan2(p.y - g0.cy, p.x - g0.cx) / D2R + 90;
    if (e.shiftKey) r = Math.round(r / 15) * 15;
    else if (snapOn()) r = Math.round(r);
    r = ((r % 360) + 360) % 360;
    setFromCenter(it, g0.cx, g0.cy, g0.w, g0.h, r);
  } else if (drag.kind === "resize") {
    ensureUndo();
    const { g0, hx, hy } = drag;
    const m = localOf(g0, p);
    let nw = g0.w;
    let nh = g0.h;
    let lcx = 0;
    let lcy = 0;
    if (hx !== 0) {
      const fx = (-hx * g0.w) / 2;
      nw = Math.max(1, hx * (m.x - fx));
      if (snapOn()) nw = Math.max(1, rnd(nw));
      lcx = fx + (hx * nw) / 2;
    }
    if (hy !== 0) {
      const fy = (-hy * g0.h) / 2;
      nh = Math.max(1, hy * (m.y - fy));
      if (snapOn()) nh = Math.max(1, rnd(nh));
      lcy = fy + (hy * nh) / 2;
    }
    const c = worldOf(g0, lcx, lcy);
    setFromCenter(it, c.x, c.y, nw, nh, g0.r);
    if (snapOn()) {
      it.v.x = rnd(it.v.x);
      it.v.y = rnd(it.v.y);
    }
  }
  renderProps();
  requestDraw();
});

function endDrag() {
  const d = drag;
  drag = null;
  if (!d) return;
  if (d.kind === "pan") {
    cv.style.cursor = spaceDown ? "grab" : "";
    return;
  }
  if (d.kind === "create") {
    const t = typeOf(d.type);
    const moved = Math.hypot(d.cur.x - d.start.x, d.cur.y - d.start.y) * view.s;
    const it = makeAt(d.type);
    if (!isRect(t)) {
      it.v.x = d.start.x;
      it.v.y = d.start.y;
    } else if (moved < 4) {
      const g = { w: it.v.w, h: it.v.h, r: it.v.r ?? 0 };
      setFromCenter(it, d.start.x, d.start.y, g.w, g.h, g.r);
      if (snapOn()) {
        it.v.x = rnd(it.v.x);
        it.v.y = rnd(it.v.y);
      }
    } else {
      const x0 = Math.min(d.start.x, d.cur.x);
      const y0 = Math.min(d.start.y, d.cur.y);
      const w = Math.max(1, Math.abs(d.cur.x - d.start.x));
      const hh = Math.max(1, Math.abs(d.cur.y - d.start.y));
      setFromCenter(it, x0 + w / 2, y0 + hh / 2, w, hh, it.v.r ?? 0);
      if (snapOn()) {
        it.v.x = rnd(it.v.x);
        it.v.y = rnd(it.v.y);
      }
    }
    pushUndo();
    doc.items.push(it);
    selected = it.id;
    save();
    renderAll();
    return;
  }
  if (d.pushed) save();
  renderAll();
}
cv.addEventListener("pointerup", endDrag);
cv.addEventListener("pointercancel", endDrag);

function updateCursor(p: Pt) {
  if (spaceDown) {
    cv.style.cursor = "grab";
    return;
  }
  if (mode === "draw") {
    cv.style.cursor = "crosshair";
    return;
  }
  const sel = selItem();
  if (sel && visible[sel.type]) {
    const hd = hitHandle(sel, p);
    if (hd) {
      cv.style.cursor = hd.kind === "rotate" ? "grab" : "pointer";
      return;
    }
  }
  cv.style.cursor = hitAll(p).length ? "move" : "default";
}

cv.addEventListener(
  "wheel",
  (e) => {
    e.preventDefault();
    const r = cv.getBoundingClientRect();
    const mx = e.clientX - r.left;
    const my = e.clientY - r.top;
    const k = Math.exp(-e.deltaY * (e.ctrlKey ? 0.01 : 0.0015));
    const ns = Math.min(80, Math.max(0.05, view.s * k));
    view.ox = mx - ((mx - view.ox) * ns) / view.s;
    view.oy = my - ((my - view.oy) * ns) / view.s;
    view.s = ns;
    requestDraw();
  },
  { passive: false },
);

// ---------------------------------------------------------------- keyboard
const isTyping = (t: EventTarget | null) =>
  t instanceof HTMLElement &&
  ["INPUT", "SELECT", "TEXTAREA"].includes(t.tagName);

window.addEventListener("keydown", (e) => {
  if (isTyping(e.target)) {
    if (e.key === "Escape") (e.target as HTMLElement).blur();
    return;
  }
  const mod = e.ctrlKey || e.metaKey;
  if (e.code === "Space") {
    e.preventDefault();
    spaceDown = true;
    cv.style.cursor = "grab";
    return;
  }
  if (mod && e.key.toLowerCase() === "z") {
    e.preventDefault();
    if (e.shiftKey) redo();
    else undo();
    return;
  }
  if (mod && e.key.toLowerCase() === "y") {
    e.preventDefault();
    redo();
    return;
  }
  if (mod && e.key.toLowerCase() === "d") {
    e.preventDefault();
    duplicateSelected();
    return;
  }
  if (mod) return;

  const k = e.key;
  if (k === "Delete" || k === "Backspace") {
    e.preventDefault();
    deleteSelected();
  } else if (k === "Escape") {
    if (mode === "draw") setMode("select");
    else {
      selected = null;
      renderAll();
    }
  } else if (k.toLowerCase() === "v") setMode("select");
  else if (k.toLowerCase() === "d") setMode("draw");
  else if (k.toLowerCase() === "f") fitView();
  else if (/^[1-9]$/.test(k)) {
    const t = TYPES[Number(k) - 1];
    if (t) chooseDrawType(t.key);
  } else if (k.startsWith("Arrow")) {
    e.preventDefault();
    const d = e.shiftKey ? 10 : 1;
    nudge(
      k === "ArrowLeft" ? -d : k === "ArrowRight" ? d : 0,
      k === "ArrowUp" ? -d : k === "ArrowDown" ? d : 0,
    );
  }
});
window.addEventListener("keyup", (e) => {
  if (e.code === "Space") {
    spaceDown = false;
    cv.style.cursor = "";
  }
});

// ---------------------------------------------------------------- toolbar
$("btn-image").addEventListener("click", () =>
  $<HTMLInputElement>("file-image").click(),
);
$("btn-import").addEventListener("click", () =>
  $<HTMLInputElement>("file-json").click(),
);
$<HTMLInputElement>("file-image").addEventListener("change", (e) => {
  const f = (e.target as HTMLInputElement).files?.[0];
  if (f) void handleFile(f);
  (e.target as HTMLInputElement).value = "";
});
$<HTMLInputElement>("file-json").addEventListener("change", (e) => {
  const f = (e.target as HTMLInputElement).files?.[0];
  if (f) void handleFile(f);
  (e.target as HTMLInputElement).value = "";
});
$("btn-paste").addEventListener("click", async () => {
  try {
    importText(await navigator.clipboard.readText(), "clipboard");
  } catch {
    toast("Could not read clipboard (permission denied?)", true);
  }
});
$("btn-copy").addEventListener("click", async () => {
  try {
    await navigator.clipboard.writeText(serialize(doc));
    toast("JSON copied to clipboard");
  } catch {
    toast("Clipboard write failed — use Download", true);
  }
});
$("btn-download").addEventListener("click", () => {
  const blob = new Blob([serialize(doc)], { type: "application/json" });
  const a = h("a", {
    href: URL.createObjectURL(blob),
    download: "level-data.json",
  });
  a.click();
  URL.revokeObjectURL(a.href);
});
$("btn-fit").addEventListener("click", fitView);
$("btn-clear").addEventListener("click", () => {
  if (!doc.items.length || confirm("Delete all items? (Ctrl+Z can undo)")) {
    mutate(() => {
      doc.items = [];
      selected = null;
    });
  }
});
$("mode-select").addEventListener("click", () => setMode("select"));
$("mode-draw").addEventListener("click", () => setMode("draw"));

// drag & drop
window.addEventListener("dragover", (e) => {
  e.preventDefault();
  stage.classList.add("drop");
});
window.addEventListener("dragleave", (e) => {
  if (e.relatedTarget === null) stage.classList.remove("drop");
});
window.addEventListener("drop", (e) => {
  e.preventDefault();
  stage.classList.remove("drop");
  for (const f of Array.from(e.dataTransfer?.files ?? [])) void handleFile(f);
});

new ResizeObserver(() => requestDraw()).observe(stage);

// ---------------------------------------------------------------- boot
loadSaved();
renderModes();
renderAll();
fitView();
void idbGet<Blob>("image").then(async (blob) => {
  if (!blob) return;
  try {
    await setImage(blob, false);
    fitView();
  } catch {
    /* ignore */
  }
});
