import type { Asset, Comp, SpriteData, GameObj, Vec2, PosComp } from "kaplay";
import { addStack } from "../stack";
import { type CarInfo } from "./info";
import { type CarType, carTypes } from "@wrum/shared";
import { camera } from "../utils/camera";

interface CarAssets {
  body: Asset<SpriteData>;
  tire: Asset<SpriteData>;
  info: CarInfo;
}

const carAssets: Record<CarType, CarAssets> = {} as Record<CarType, CarAssets>;

function loadCarAssets(type: CarType, info: CarInfo): CarAssets {
  const body = loadSprite(`${type}_body`, `sprites/cars/${type}/body.png`, {
    sliceX: info.bodyLayers,
    sliceY: 1,
  });

  const tire = loadSprite(`${type}_tire`, `sprites/cars/${type}/tire.png`, {
    sliceX: info.tireLayers,
    sliceY: 1,
  });

  return { body, tire, info };
}

export async function loadAllCarAssets() {
  for (const type of carTypes) {
    const res = await fetch(`sprites/cars/${type}/info.json`);
    const info = (await res.json()) as CarInfo;

    carAssets[type] = loadCarAssets(type, info);
  }
}

export function addCar(x: number, y: number, identifier: string, type: CarType) {
  const assets = carAssets[type];
  const info = assets.info;

  const c = add([pos(x, y), anchor("center"), car(type, identifier, info, x, y)]);
  for (let i = 0; i < assets.info.tireOffsets.length; i++) {
    addStack(
      c,
      assets.info.tireOffsets[i]!.x,
      assets.info.tireOffsets[i]!.y,
      assets.tire,
      "center",
      [tire(i)],
    );
  }

  addStack(c, 0, 0, assets.body, undefined, undefined, 10);

  return c;
}

interface TireComp extends Comp {
  index: number;
}

function tire(index: number): TireComp {
  return {
    id: "tire",
    index,
  };
}

export interface CarComp extends Comp {
  type: CarType;
  rotation: number;
  tireRotation: number;
  ident: string;
  info: CarInfo;
  // it's "actualPos"and not for example "worldPos", kaplay uses "worldPos" already
  actualPos: Vec2;
  renderRotation: number;
}

function car(type: CarType, ident: string, info: CarInfo, x: number, y: number): CarComp {
  return {
    id: "car",
    type,
    rotation: 0,
    tireRotation: 0,
    info,
    actualPos: vec2(x, y),
    renderRotation: 0,
    update(this: GameObj<CarComp | PosComp>) {
      const camPos = camera.apply(this.actualPos.x, this.actualPos.y, this.rotation);
      this.pos.x = camPos.x;
      this.pos.y = camPos.y;
      this.renderRotation = camPos.rot;

      const assets = carAssets[this.type];

      this.children.forEach((child) => {
        if (child.has("stack")) {
          if (child.has("tire")) {
            const t = child as GameObj<TireComp>;

            const baseOffset = assets.info.tireOffsets[t.index] ?? {
              x: 0,
              y: 0,
            };
            const rotated = rotateVec(vec2(baseOffset.x, baseOffset.y), this.rotation);
            child.pos = rotated;

            const shouldRotateIndependently = this.info.rotateTires[t.index];
            child.rotation = shouldRotateIndependently
              ? this.tireRotation + this.rotation
              : this.rotation;
          } else {
            child.rotation = this.rotation;
          }
        }
      });
    },
    ident,
  };
}

function rotateVec(v: Vec2, angleDeg: number) {
  const rad = (angleDeg * Math.PI) / 180;
  const cos = Math.cos(rad);
  const sin = Math.sin(rad);

  return vec2(v.x * cos - v.y * sin, v.x * sin + v.y * cos);
}
