import { defineConfig } from "vite";

export default defineConfig({
  base: "./",
  server: {
    port: 3001,
  },
  build: {
    sourcemap: true,
    rollupOptions: {
      output: {
        codeSplitting: {
          groups: [
            {
              name(id) {
                if (id.includes("node_modules")) return "vendor";
                if (id.includes("kaplay")) return "kaplay";
                return null;
              },
            },
          ],
        },
      },
    },
  },
  plugins: [],
});
