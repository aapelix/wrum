import kaplay from "kaplay";
import "kaplay/global";
import { addCar, loadAllCarAssets } from "./car/car";

const apiUrl = import.meta.env.VITE_API_URL;

if (!apiUrl) {
  throw new Error("VITE_API_URL is not defined");
}

const protocol = location.protocol === "https:" ? "wss:" : "ws:";
const ws = new WebSocket(`${protocol}//${apiUrl}/conn`);

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
