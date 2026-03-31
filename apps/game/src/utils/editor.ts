import type { GameObj } from "kaplay";

const tilesheet = "sprites/tiles.png";
const tileSize = 6;

function pickFile(): Promise<string> {
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = ".json";

    input.onchange = () => {
      const file = input.files?.[0];
      if (!file) return;

      const reader = new FileReader();
      reader.onload = () => resolve(reader.result as string);
      reader.readAsText(file);
    };

    input.click();
  });
}

export function loadEditorScene() {
  let selectedIndex = 0;
  let cols = 0;
  let rows = 0;

  const selectedTile = { x: 0, y: 0 };

  let tiles: GameObj[] = [];

  async function loadMapFromFile() {
    const text = await pickFile();
    const data = JSON.parse(text);

    tiles.forEach((t) => destroy(t));
    tiles = [];

    for (const t of data.tiles) {
      const tile = add([
        sprite("tiles", { frame: t.tile }),
        pos(t.x * tileSize, t.y * tileSize),
        anchor("topleft"),
      ]);
      tiles.push(tile);
    }
  }

  scene("editor", async () => {
    const img = await loadSprite(null, tilesheet);
    cols = Math.floor(img.width / tileSize);
    rows = Math.floor(img.height / tileSize);

    onKeyPress("l", () => {
      loadMapFromFile();
    });

    loadSprite("tiles", tilesheet, {
      sliceX: cols,
      sliceY: rows,
    });

    for (let y = 0; y < rows; y++) {
      for (let x = 0; x < cols; x++) {
        const tile = add([
          sprite("tiles", { frame: y * cols + x }),
          pos(x * tileSize + 6, y * tileSize + 6),
          anchor("topleft"),
          area(),
        ]);
        tile.onMouseDown(() => {
          if (tile.isHovering()) {
            selectedIndex = y * cols + x;
          }
        });
      }
    }

    const selectedTileBorder = add([
      rect(tileSize, tileSize, { fill: false }),
      outline(1, Color.BLACK),
      pos(selectedTile.x, selectedTile.y),
      color(1, 0, 0),
      anchor("topleft"),
      z(10),
    ]);
    onMouseMove((pos) => {
      selectedTile.x = Math.floor(pos.x / tileSize) * tileSize;
      selectedTile.y = Math.floor(pos.y / tileSize) * tileSize;
      selectedTileBorder.pos = vec2(selectedTile.x, selectedTile.y);
    });

    onMouseDown((m) => {
      if (m === "right") {
        const existing = tiles.find(
          (t) => t.pos.x === selectedTile.x && t.pos.y === selectedTile.y,
        );
        if (existing) {
          destroy(existing);
          tiles = tiles.filter((t) => t !== existing);
        }
        return;
      }

      const existing = tiles.find(
        (t) => t.pos.x === selectedTile.x && t.pos.y === selectedTile.y,
      );
      if (existing) {
        if (existing.frame === selectedIndex) {
          return;
        }

        destroy(existing);
        tiles = tiles.filter((t) => t !== existing);
      }
      const tile = add([
        sprite("tiles", { frame: selectedIndex }),
        pos(selectedTile.x, selectedTile.y),
        anchor("topleft"),
      ]);
      tiles.push(tile);
    });

    onKeyPress("right", () => {
      selectedIndex = (selectedIndex + 1) % (cols * rows);
    });

    onKeyPress("left", () => {
      selectedIndex = (selectedIndex - 1 + cols * rows) % (cols * rows);
    });

    onKeyPress("up", () => {
      selectedIndex = (selectedIndex - cols + cols * rows) % (cols * rows);
    });

    onKeyPress("down", () => {
      selectedIndex = (selectedIndex + cols) % (cols * rows);
    });

    onKeyPress("e", async () => {
      const data = {
        tiles: tiles.map((t) => ({
          tile: t.frame,
          x: Math.round(t.pos.x / tileSize),
          y: Math.round(t.pos.y / tileSize),
        })),
      };

      await navigator.clipboard.writeText(JSON.stringify(data));
      debug.log("Copied tile JSON");
    });
  });
}
