const POS_SMOOTHING = 60;
const ROT_SMOOTHING = 100;

export class Camera {
  x = 0;
  y = 0;
  rotation = 0;

  update(targetX: number, targetY: number, targetRotation: number) {
    const delta = dt();

    this.x += (targetX - this.x) * POS_SMOOTHING * delta;
    this.y += (targetY - this.y) * POS_SMOOTHING * delta;

    const rotDiff = ((targetRotation - this.rotation + 540) % 360) - 180;
    this.rotation += rotDiff * ROT_SMOOTHING * delta;
  }

  apply(x: number, y: number, rot: number) {
    const translatedX = x - this.x;
    const translatedY = y - this.y;

    const rad = (-this.rotation * Math.PI) / 180;
    const cos = Math.cos(rad);
    const sin = Math.sin(rad);

    const rotatedX = translatedX * cos - translatedY * sin;
    const rotatedY = translatedX * sin + translatedY * cos;

    return {
      x: rotatedX + width() / 2,
      y: rotatedY + height() / 2,
      rot: rot - this.rotation,
    };
  }
}

export const camera = new Camera();
