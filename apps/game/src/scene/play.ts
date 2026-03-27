import { addButton } from "../ui/button";
import { addInput } from "../ui/input";
import { trpc } from "../trpc";

export function loadPlayScene() {
  const w = width();
  const h = height();

  scene("play", () => {
    addButton(w / 2, h / 2 - 25, 50, 15, "Join", () => go("lobbyCode"), 9);
    addButton(
      w / 2,
      h / 2 - 5,
      50,
      15,
      "Create",
      () => {
        trpc.game.create
          .mutate({
            carType: "red",
          })
          .then((res) => {
            go("lobby", res.lobbyId, res.playerId);
          })
          .catch((err) => {
            go("error", err);
          });
      },
      9,
    );

    addButton(
      w / 2,
      h / 2 + 52.5,
      50,
      15,
      "Back",
      () => {
        go("main");
      },
      9,
      undefined,
      "secondary",
    );
  });
  scene("lobbyCode", () => {
    let code = "";

    addInput(
      w / 2,
      h / 2 - 10,
      50,
      15,
      "xxxxxx",
      (s) => {
        code = s;
      },
      "text",
      true,
      6,
      (s) => s.toLowerCase(),
    );
    addButton(
      w / 2,
      h / 2 + 10,
      50,
      15,
      "Join",
      () => {
        const result = code.toLowerCase().replace(/[^a-z0-9]/g, "");

        trpc.game.join
          .mutate({
            lobbyId: result,
            carType: "red",
          })
          .then((playerId) => {
            go("lobby", result, playerId);
          })
          .catch((err) => {
            go("error", err);
          });
      },
      9,
    );

    addButton(
      w / 2,
      h / 2 + 35,
      50,
      15,
      "Back",
      () => {
        go("play");
      },
      9,
      undefined,
      "secondary",
    );
  });
}
