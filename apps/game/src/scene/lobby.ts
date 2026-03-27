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
  });
}
