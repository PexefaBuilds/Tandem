import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "@fontsource/instrument-sans/400.css";
import "@fontsource/instrument-sans/500.css";
import "@fontsource/instrument-sans/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";
import "./styles/tokens.css";
import "./styles/components.css";
import { applyTheme, savedTheme, syncIcon } from "./theme";
import App from "./App";
import { Flyout } from "./Flyout";

applyTheme(savedTheme());
matchMedia("(prefers-color-scheme: dark)").addEventListener("change", syncIcon);
createRoot(document.getElementById("root")!).render(
  <StrictMode>
    {location.hash === "#tray" ? <Flyout /> : <App />}
  </StrictMode>,
);
