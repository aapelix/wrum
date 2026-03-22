import { Asset, Comp, SpriteData, GameObj } from "kaplay";
import "kaplay/global";

export function addStack(x: number, y: number, asset: Asset<SpriteData>) {
  asset.then((data) => {
    const layers = data.frames.length;
    const s = add([pos(x, y), anchor("center"), stack()]);

    for (let i = 0; i < layers; i++) {
      s.add([
        pos(0, -i * 0.75),
        rotate(),
        anchor("center"),
        sprite(asset, { frame: i }),
      ]);
    }

    return stack;
  });
}

interface StackComp extends Comp {
  rotation: number;
}

function stack(): StackComp {
  return {
    id: "stack",
    rotation: 0,
    update(this: GameObj<StackComp>) {
      this.rotation += 20 * dt();

      this.children.forEach((child) => {
        child.angle = this.rotation;
      });
    },
  };
}
