import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import tauriConf from "./src-tauri/tauri.conf.json" with { type: "json" };
import pkg from "./package.json" with { type: "json" };

const CHUNK_WARN_KB = 500;
const MATHLIVE_CHUNK_WARN_KB = 900;
const DEV_PORT = Number(new URL(tauriConf.build.devUrl).port);

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

export default defineConfig({
  plugins: [react(), chunkSizeGuard()],
  define: { __APP_VERSION__: JSON.stringify(pkg.version) },
  clearScreen: false,
  server: { port: DEV_PORT, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
  build: {
    target: "es2022",
    sourcemap: true,
    chunkSizeWarningLimit: MATHLIVE_CHUNK_WARN_KB,
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [{ name: "mathlive", test: /[\\/]node_modules[\\/]mathlive[\\/]/ }],
        },
      },
    },
  },
});
