import { addBox } from "./box";

export function addInput(
  x: number,
  y: number,
  w: number,
  h: number,
  placeholder: string,
  onChange: (value: string) => void,
  type: "text" | "password" = "text"
) {
  const input = add([
    pos(x, y),
    text(placeholder, {
      size: 9,
    }),
    anchor("center"),
    textInput(),
  ]);

  input.hasFocus = false;

  input.onUpdate(() => {
    if (input.hasFocus) {
      onChange(input.text);
    }
  });

  const bg = addBox(x, y, w, h);

  bg.onMouseDown(() => {
    if (bg.isHovering()) {
      input.hasFocus = true;
      bg.color = color(19, 96, 178).color;
    } else {
      input.hasFocus = false;
      bg.color = color(0, 122, 255).color;

      if (input.text === "") {
        input.text = placeholder;
      }
    }
  });

  if (type === "password") {
    input.onUpdate(() => {
      if (input.hasFocus && input.typedText !== "") {
        input.text = "*".repeat(input.text.length);
      }
    });
  }

  return input;
}
