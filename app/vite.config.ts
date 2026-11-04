import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";

/** Vite's default limit, kept for every chunk except MathLive's (see below). */
const CHUNK_WARN_KB = 500;

/**
 * MathLive ships as one prebuilt, minified ES module (mathlive.min.mjs, about
 * 850 kB) that no bundler can split. It goes in its own `mathlive` chunk, so
 * the app's entry chunk stays small, and it is the only chunk allowed over the
 * usual limit: Vite's global `chunkSizeWarningLimit` is raised just above it,
 * and this plugin keeps the 500 kB warning for every other chunk.
 */
function chunkSizeGuard(): Plugin {
  return {
    name: "separatrix-chunk-size-guard",
    apply: "build",
    generateBundle(_options, bundle) {
      for (const out of Object.values(bundle)) {
        if (out.type !== "chunk" || out.name === "mathlive") continue;
        const kb = new TextEncoder().encode(out.code).length / 1000;
        if (kb > CHUNK_WARN_KB) {
          this.warn(`chunk ${out.fileName} is ${kb.toFixed(0)} kB, over the ${CHUNK_WARN_KB} kB limit`);
        }
      }
    },
  };
}

// Tauri expects a fixed port and does not want Vite to clear the Rust logs.
export default defineConfig({
  plugins: [react(), chunkSizeGuard()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: {
    target: "es2022",
    sourcemap: true,
    // Only the mathlive chunk can reach this; chunkSizeGuard checks the rest.
    chunkSizeWarningLimit: 900,
    rolldownOptions: {
      output: {
        // MathLive is still loaded at startup (every equation is typeset with it),
        // but as a separate file the HTML preloads next to the entry chunk.
        codeSplitting: {
          groups: [{ name: "mathlive", test: /[\\/]node_modules[\\/]mathlive[\\/]/ }],
        },
      },
    },
  },
});
