export function addBox(
  x: number,
  y: number,
  w: number,
  h: number,
  radius: number = 4,
  colorScheme: "default" | "secondary" = "default",
) {
  const colorSchemes = {
    default: {
      bg: color(0, 122, 255),
      outline: color(255, 255, 255),
      text: color(255, 255, 255),
    },
    secondary: {
      bg: color(19, 96, 178),
      outline: color(255, 255, 255),
      text: color(255, 255, 255),
    },
  };

  const colors = colorSchemes[colorScheme];

  const box = add([
    pos(x, y),
    anchor("center"),
    rect(w, h, { radius }),
    colors.bg,
    outline(1.5, colors.outline.color),
    area(),
    layer("ui"),
    "box",
  ]);

  return box;
}
