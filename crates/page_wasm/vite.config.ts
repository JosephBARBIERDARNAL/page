import wasm from "vite-plugin-wasm";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [wasm()],
  esbuild: {
    target: "esnext",
  },
  build: {
    target: "esnext",
  },
});
