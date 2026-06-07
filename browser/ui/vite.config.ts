import { defineConfig } from "vite";
import react from "@vitejs/plugin-react-swc";
import tailwindcss from "@tailwindcss/vite";
import prismjs from "vite-plugin-prismjs";
import { compression } from "vite-plugin-compression2";

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    compression(),
    prismjs({ languages: ["python", "rust"] }),
  ],
  build: {
    // The server embeds only index.html/index.css/index.js, so inline all
    // assets (fonts, svg) as data URIs rather than emitting separate files.
    assetsInlineLimit: 4 * 1024 * 1024,
  },
});
