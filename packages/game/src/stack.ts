import type {
  Asset,
  Comp,
  SpriteData,
  GameObj,
  Anchor,
  Vec2,
  PosComp,
  ZComp,
} from "kaplay";

export function addStack(
  parent: GameObj,
  x: number,
  y: number,
  asset: Asset<SpriteData>,
  anc?: Vec2 | Anchor,
  attr: Comp[] = [],
  baseZ: number = 1
) {
  asset.then((data) => {
    const layers = data.frames.length;
    const s = parent.add([
      pos(x, y),
      anchor("center"),
      stack(baseZ),
      z(baseZ),
      ...attr,
    ]);

    for (let i = 0; i < layers; i++) {
      s.add([
        pos(0, -i * 0.75),
        rotate(),
        anchor(anc ?? "center"),
        sprite(asset, { frame: i }),
        z(1),
      ]);
    }

    return stack;
  });
}

interface StackComp extends Comp {
  rotation: number;
  baseZ: number;
}

function stack(baseZ: number = 1): StackComp {
  return {
    id: "stack",
    baseZ,
    rotation: 0,
    update(this: GameObj<StackComp | PosComp | ZComp>) {
      this.z = -this.worldPos()!.y + this.baseZ;

      this.children.forEach((child) => {
        child.angle = this.rotation;

        child.z = this.z;
      });
    },
  };
}
