import { defineConfig } from "vite";
import wasm from "vite-plugin-wasm";

export default defineConfig({
  plugins: [wasm()],
  base: "./",
  build: {
    emptyOutDir: true,
    lib: {
      entry: "src/main.ts",
      formats: ["es"],
      fileName: "validator-demo",
    },
    outDir: "../../docs/javascripts/validator-demo",
    rollupOptions: {
      output: {
        assetFileNames: "[name][extname]",
      },
    },
  },
});
