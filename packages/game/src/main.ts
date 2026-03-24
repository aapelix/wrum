import kaplay from "kaplay";
import "kaplay/global";
import "./auth/scene/login";
import { loadAllCarAssets } from "./car/car";
import { loadScenes } from "./scene";
import { authClient } from "./auth/client";

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
  font: "happy",
});

setLayers(["ui", "game"], "game");

loadBitmapFont("happy", "/fonts/happy_28x36.png", 28, 36);

loadRoot("./");
loadScenes();
await loadAllCarAssets();

const { data: session, error } = await authClient.getSession();

if (error) {
  go("error", error);
}

if (session) {
  go("main");
} else {
  go("login");
}
