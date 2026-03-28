import type { Player } from "@wrum/shared";
import { trpc } from "../trpc";
import type { GameObj, PosComp } from "kaplay";
import { addCar, type CarComp } from "../car/car";
import { camera } from "../utils/camera";

const LERP_SPEED = 20;

export function loadLobbyScene() {
  const w = width();
  const h = height();

  scene("lobby", (lobbyId: string, playerId: string) => {
    const players = new Map<string, Player>();
    const cars = new Map<string, GameObj<CarComp | PosComp>>();

    let lastThrottle = 0;
    let lastSteering = 0;

    let throttle = 0;
    let steering = 0;

    add([
      text(lobbyId, {
        size: 10,
      }),
      pos(w / 2, h / 2),
      anchor("center"),
    ]);

    add([
      text(playerId, {
        size: 10,
      }),
      pos(w / 2, h / 2 + 20),
      anchor("center"),
    ]);

    const sub = trpc.game.serverUpdate.subscribe(undefined, {
      onData({ type, data }) {
        if (type === "update") {
          for (const player of data.players) {
            players.set(player.id, player);
          }
        }

        if (type === "leave") {
          players.delete(data.playerId);
        }

        if (type === "close") {
          go("main");
        }
      },
      onError(err) {
        console.error("Subscription error:", err);
      },
    });

    onUpdate(() => {
      if (isKeyDown(["up", "w"])) {
        throttle = 1;
      } else if (isKeyDown(["down", "s"])) {
        throttle = -1;
      } else {
        throttle = 0;
      }

      if (isKeyDown(["left", "a"])) {
        steering = -1;
      } else if (isKeyDown(["right", "d"])) {
        steering = 1;
      } else {
        steering = 0;
      }

      const delta = dt();
      for (const [id, player] of players) {
        let car = cars.get(id);
        if (!car) {
          car = addCar(player.x, player.y, player.id, player.carType);
          cars.set(id, car);
        } else {
          car.actualPos.x = lerp(car.actualPos.x, player.x, delta * LERP_SPEED);
          car.actualPos.y = lerp(car.actualPos.y, player.y, delta * LERP_SPEED);

          car.rotation = lerp(car.rotation, player.rotation, delta * LERP_SPEED);
          car.tireRotation = lerp(car.tireRotation, player.tireRotation, delta * LERP_SPEED);
        }
      }

      if (steering !== lastSteering || throttle !== lastThrottle) {
        trpc.game.clientUpdate.mutate(
          {
            steering,
            throttle,
          },
          { context: { useWS: true } },
        );

        lastSteering = steering;
        lastThrottle = throttle;
      }

      const player = players.get(playerId);
      if (player) {
        camera.update(player.x, player.y, player.rotation);
      }
    });

    onSceneLeave(() => {
      sub.unsubscribe();
    });
  });
}
