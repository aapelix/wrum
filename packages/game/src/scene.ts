import { loadAuthScenes } from "./auth/scenes";

export function loadScenes() {
  loadAuthScenes();
  scene("main", () => {
    add([text("Main Scene"), pos(10, 10)]);
  });
  scene("error", (err) => {
    add([text(`Error: ${err.message}`), pos(10, 10)]);
  });
}
