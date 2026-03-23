import kaplay from "kaplay";
import "kaplay/global";
import { addCar, loadAllCarAssets } from "./car/car";

const ws = new WebSocket("ws://localhost:8080");
ws.addEventListener("open", () => {
  debug.log("WebSocket connection opened");
});

kaplay({
  scale: 4,
  crisp: true,
});

loadRoot("./");
await loadAllCarAssets();

const car = addCar(120, 120, "test_id", "red");
car.onKeyPress((key) => {
  debug.log("pressed", key);
});

onClick(() => addKaboom(mousePos()));
