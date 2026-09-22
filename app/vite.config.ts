import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Fixed port because tauri.conf.json's devUrl names it; a moving port means a
// blank window with no obvious cause.
//
// 1421 rather than Tauri's default 1420, which every scaffolded Tauri app on
// this machine also claims — whichever starts second fails with "port already
// in use" and no hint as to which project holds it. Taking a port of our own
// costs nothing and stops pattern-engine from being the one that loses.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1421, strictPort: true },
  build: { target: "es2022", sourcemap: true },
});
