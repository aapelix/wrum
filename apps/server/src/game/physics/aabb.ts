import type { Player } from "../lobby/player";

export function collisionOverlap(a: Player, b: Player) {
  const ahw = a.collider.width / 2;
  const ahh = a.collider.height / 2;
  const bhw = b.collider.width / 2;
  const bhh = b.collider.height / 2;

  const ax1 = a.x - ahw;
  const ax2 = a.x + ahw;
  const ay1 = a.y - ahh;
  const ay2 = a.y + ahh;

  const bx1 = b.x - bhw;
  const bx2 = b.x + bhw;
  const by1 = b.y - bhh;
  const by2 = b.y + bhh;

  return ax1 < bx2 && ax2 > bx1 && ay1 < by2 && ay2 > by1;
}

export function resolveCollision(a: Player, b: Player) {
  const dx = b.x - a.x;
  const dy = b.y - a.y;

  const overlapX = a.collider.width / 2 + b.collider.width / 2 - Math.abs(dx);
  const overlapY = a.collider.height / 2 + b.collider.height / 2 - Math.abs(dy);

  if (overlapX <= 0 || overlapY <= 0) return;

  if (overlapX < overlapY) {
    const push = overlapX * (dx > 0 ? -1 : 1);
    a.x += push * 0.5;
    b.x -= push * 0.5;
  } else {
    const push = overlapY * (dy > 0 ? -1 : 1);
    a.y += push * 0.5;
    b.y -= push * 0.5;
  }
}

export function handleCollisions(players: Player[]) {
  for (let i = 0; i < players.length; i++) {
    for (let j = i + 1; j < players.length; j++) {
      const a = players[i]!;
      const b = players[j]!;

      if (collisionOverlap(a, b)) {
        resolveCollision(a, b);
      }
    }
  }
}
