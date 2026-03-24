import { addBox } from "./box";

export function addButton(
  x: number,
  y: number,
  w: number,
  h: number,
  content: string,
  onClick: () => void,
  fontSize: number = 16,
  radius: number = 4,
  colorScheme: "default" | "secondary" = "default"
) {
  const btn = addBox(x, y, w, h, radius, colorScheme);

  const btnBg = btn.color;

  add([
    text(content, { size: fontSize }),
    pos(x, y),
    anchor("center"),
    layer("ui"),
    color(255, 255, 255),
  ]);

  btn.onClick(() => {
    onClick();
  });

  btn.onMouseDown(() => {
    if (btn.isHovering()) {
      btn.color = btnBg.darken(30);
    } else {
      btn.color = btnBg;
    }
  });

  btn.onMouseRelease(() => {
    btn.color = btnBg;
  });

  return btn;
}
