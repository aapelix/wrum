import type { User } from "better-auth";
import { loadAuthScenes } from "./auth/scenes";
import { authClient } from "./auth/client";
import { addButton } from "./ui/button";

export function loadScenes() {
  loadAuthScenes();
  scene("main", (user: User) => {
    add([
      text("coming soon", {
        size: 10,
      }),
      pos(10, 50),
    ]);

    add([
      text(`${user.email}`, {
        size: 10,
      }),
      pos(10, 70),
    ]);

    addButton(
      width() - 30,
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
      "secondary"
    );
  });
  scene("error", (err) => {
    add([
      text(`Error: ${err.message}`, {
        size: 10,
      }),
      pos(10, 10),
    ]);
  });
}
