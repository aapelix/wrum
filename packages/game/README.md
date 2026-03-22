# Folder structure

- `src` - source code for your kaplay project
- `dist` - distribution folder, contains your index.html, built js bundle and static assets


## Development

```sh
$ bun run dev
```

will start a dev server at http://localhost:8000

## Distribution

```sh
$ bun run build
```

will build your js files into `dist/`

```sh
$ bun run zip
```

will build your game and package into a .zip file, you can upload to your server or itch.io / newground etc.