import { loadAuthScenes } from "./auth/scenes";
import { authClient } from "./auth/client";
import { addButton } from "./ui/button";
import { initWs } from "./ws";
import { loadPlayScene } from "./scene/play";
import { getUser } from "./user";
import { loadLobbyScene } from "./scene/lobby";

export function loadScenes() {
  loadAuthScenes();
  loadPlayScene();
  loadLobbyScene();

  const w = width();
  const h = height();

  scene("main", async () => {
    const user = await getUser();
    if (!user) {
      go("login");
      return;
    }

    initWs();

    add([
      text("wrum", {
        size: 10,
      }),
      pos(w / 2, h / 2 - 20),
      anchor("center"),
    ]);

    add([
      text(`${user.email}`, {
        size: 10,
        align: "right",
      }),
      pos(w - 10, 30),
      anchor("right"),
    ]);

    addButton(
      w - 30,
      12,
      50,
      15,
      "Logout",
      async () => {
        const { error } = await authClient.signOut();

        if (error) {
          go("error", error);
        } else {
          go("login");
        }
      },
      9,
      undefined,
      "secondary",
    );

    addButton(w / 2, h / 2, 50, 15, "Play", () => go("play", user), 9);
  });
  scene("error", (err) => {
    add([
      text(`Error: ${err}`, {
        size: 10,
      }),
      pos(w / 2, h / 2),
      anchor("center"),
    ]);
  });
}
