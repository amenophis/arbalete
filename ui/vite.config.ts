import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Fixed file names: the app embeds these files at compile time.
export default defineConfig({
  plugins: [react()],
  build: {
    rollupOptions: {
      output: {
        entryFileNames: "assets/index.js",
        chunkFileNames: "assets/[name].js",
        assetFileNames: "assets/[name][extname]",
      },
    },
  },
});
