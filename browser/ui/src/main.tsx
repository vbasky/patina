import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { nonNull } from "./core/util";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/500.css";
import "@fontsource/jetbrains-mono/700.css";
import "./index.css";
import App from "./App.tsx";
import { initTheme } from "./core/theme";

initTheme();

createRoot(nonNull(document.getElementById("root"))).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
