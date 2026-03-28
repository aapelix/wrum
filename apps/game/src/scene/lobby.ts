import { trpc } from "../trpc";

export function loadLobbyScene() {
  const w = width();
  const h = height();

  scene("lobby", (lobbyId: string, playerId: string) => {
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
      onData(data) {
        console.log("Received update:", data);
      },
      onError(err) {
        console.error("Subscription error:", err);
      },
    });

    onSceneLeave(() => {
      sub.unsubscribe();
    });
  });
}
