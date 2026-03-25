import kaplay from "kaplay";
import "kaplay/global";
import "./auth/scene/login";
import { loadAllCarAssets } from "./car/car";
import { loadScenes } from "./scene";
import { getUser } from "./user";

kaplay({
  scale: 4,
  crisp: true,
  font: "happy",
});

setLayers(["ui", "game"], "game");

loadBitmapFont("happy", "/fonts/happy_28x36.png", 28, 36);

loadRoot("./");
loadScenes();
await loadAllCarAssets();

const user = await getUser();

if (user) {
  go("main");
}
