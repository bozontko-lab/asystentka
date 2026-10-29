import { fileURLToPath } from "node:url";

import { defineConfig } from "vite";

// Konfiguracja zgodna z szablonem Tauri 2: stały port i brak czyszczenia
// konsoli, żeby widać było błędy z Rusta.
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1421 } : undefined,
    watch: { ignored: ["**/src-tauri/**", "**/.piaskownica/**"] },
  },
  build: {
    target: "es2022",
    // WebView2 na Windows to Chromium, więc nie potrzeba transpilacji dla starych przeglądarek.
    sourcemap: false,
    rollupOptions: {
      // Widget i okno ustawień to dwie strony tej samej aplikacji.
      input: {
        main: fileURLToPath(new URL("index.html", import.meta.url)),
        ustawienia: fileURLToPath(new URL("ustawienia.html", import.meta.url)),
      },
    },
  },
});
