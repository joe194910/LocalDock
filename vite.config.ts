import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  build: {
    outDir: "dist/client",
    emptyOutDir: true,
  },
  clearScreen: false,
  optimizeDeps: {
    include: ["react", "react-dom/client", "lucide-react"],
  },
  server: {
    host: "0.0.0.0",
    port: 4173,
    strictPort: false,
    watch: {
      ignored: ["**/.tools/**", "**/src-tauri/target/**"],
    },
    allowedHosts: ["terminal.local"],
    warmup: {
      clientFiles: ["./src/main.tsx"],
    },
  },
  plugins: [react()],
});
