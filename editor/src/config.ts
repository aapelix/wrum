/**
 * Type definitions. To add a new type or a new property, edit this file only.
 *
 * - `key`      : the top-level key in the exported JSON.
 * - `columns`  : the order of values in each exported array row.
 *                Special names: x, y, w, h, r (rotation, degrees).
 *                Any other name is a custom property (shown in the properties panel and as a label).
 *                No w/h columns => the type is a point. No `r` column => it can't be rotated.
 * - `anchor`   : what (x, y) refers to inside the rect, as a fraction of its size.
 *                [0.5, 0.5] = center (default), [0, 0] = top-left corner. Rotation pivots on the center
 *                of the rect, and (x, y) is recomputed from the anchor.
 * - `defaultSize` : size of a rect created by a plain click.
 */
export interface Column {
  name: string;
  default: number;
  step?: number;
  options?: string[];
}

export interface TypeDef {
  key: string;
  label: string;
  color: string; // #rrggbb
  columns: Column[];
  geometry?: "rect" | "point" | "line";
  anchor?: [number, number];
  defaultSize?: [number, number];
}

export const DEFAULT_ANCHOR: [number, number] = [0.5, 0.5];

const col = (name: string, def = 0, step?: number): Column => ({
  name,
  default: def,
  step,
});
const choice = (name: string, options: string[]): Column => ({
  name,
  default: 0,
  options,
});

export const TYPES: TypeDef[] = [
  {
    key: "checkpoints",
    label: "Checkpoint",
    color: "#ffd43b",
    columns: [
      col("x"),
      col("y"),
      col("w"),
      col("h"),
      col("r"),
      col("index", 0),
    ],
    defaultSize: [25, 166],
  },
  {
    key: "water",
    label: "Water",
    color: "#339af0",
    columns: [col("x"), col("y"), col("w"), col("h")],
    defaultSize: [128, 128],
  },
  {
    key: "spawns",
    label: "Spawn",
    color: "#51cf66",
    columns: [col("x"), col("y"), col("r")],
  },
  {
    key: "colliders",
    label: "Collider",
    color: "#ff6b6b",
    columns: [col("x"), col("y"), col("w"), col("h"), col("r")],
    defaultSize: [64, 16],
  },
  {
    key: "jump_pads",
    label: "Jump pad",
    color: "#f06595",
    columns: [
      col("x"),
      col("y"),
      col("w"),
      col("h"),
      col("r"),
      col("power", 1, 0.1),
    ],
    defaultSize: [48, 12],
  },
  {
    key: "speed_pads",
    label: "Speed pad",
    color: "#e678e8",
    columns: [
      col("x"),
      col("y"),
      col("w"),
      col("h"),
      col("r"),
      col("tier", 1, 1),
    ],
    defaultSize: [48, 12],
  },
  {
    key: "props",
    label: "Prop",
    color: "#fcc419",
    columns: [
      col("x"),
      col("y"),
      col("r"),
      choice("kind", ["fence", "fence_end", "tires"]),
    ],
  },
];
