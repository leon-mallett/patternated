import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Fixed port because tauri.conf.json's devUrl names it; a moving port means a
// blank window with no obvious cause.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: { target: "es2022", sourcemap: true },
});
