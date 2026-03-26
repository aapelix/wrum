export function loadLobbyScene() {
  const w = width();
  const h = height();

  scene("lobby", (lobbyId: string) => {
    add([
      text(lobbyId, {
        size: 10,
      }),
      pos(w / 2, h / 2),
      anchor("center"),
    ]);
  });
}
