import type { Player } from "@wrum/shared";
import { trpc } from "../trpc";
import type { GameObj, PosComp } from "kaplay";
import { addCar, type CarComp } from "../car/car";

export function loadLobbyScene() {
  const w = width();
  const h = height();

  scene("lobby", (lobbyId: string, playerId: string) => {
    const players = new Map<string, Player>();
    const cars = new Map<string, GameObj<CarComp | PosComp>>();

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
            if (!players.has(player.id)) {
              continue;
            }

            players.set(player.id, player);
          }
        }

        if (type === "join") {
          for (const player of data.players) {
            players.set(player.id, player);
          }
        }

        if (type === "otherJoined") {
          if (data.player.id !== playerId) {
            players.set(data.player.id, data.player);
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
      for (const [id, player] of players) {
        let car = cars.get(id);
        if (!car) {
          car = addCar(player.x, player.y, player.id, player.carType);
          cars.set(id, car);
        } else {
          car.pos.x = player.x;
          car.pos.y = player.y;
          car.rotation = player.rotation;
        }
      }
    });

    onSceneLeave(() => {
      sub.unsubscribe();
    });
  });
}
