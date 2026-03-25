import type { User } from "better-auth";
import { addButton } from "../ui/button";
import { addInput } from "../ui/input";
import { send } from "../ws";

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
        send({
          type: "create",
          data: {
            carType: "red",
          },
        });
      },
      9
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
      "secondary"
    );
  });
  scene("lobbyCode", (user: User) => {
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
      (s) => s.toLowerCase()
    );
    addButton(
      w / 2,
      h / 2 + 10,
      50,
      15,
      "Join",
      () => {
        const result = code.toLowerCase().replace(/[^a-z0-9]/g, "");
        send({
          type: "join",
          data: {
            carType: "red",
            lobbyId: result,
          },
        });
      },
      9
    );

    addButton(
      w / 2,
      h / 2 + 35,
      50,
      15,
      "Back",
      () => {
        go("play", user);
      },
      9,
      undefined,
      "secondary"
    );
  });
}
