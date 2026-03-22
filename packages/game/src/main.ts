import kaplay from "kaplay";
import "kaplay/global";
import { addStack } from "./stack";

kaplay({
  scale: 4,
  crisp: true,
});

loadRoot("./"); // A good idea for Itch.io publishing later
const red_body = loadSprite("red_body", "sprites/cars/red/body.png", {
  sliceX: 7,
  sliceY: 1,
});

addStack(120, 120, red_body);

onClick(() => addKaboom(mousePos()));
