import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./app/App";
import { I18nProvider } from "./i18n";
import { DisplayPreferencesProvider } from "./preferences/DisplayPreferences";
import "./styles/global.css";

const root = document.getElementById("root");

if (!root) {
  throw new Error("Application root not found");
}

createRoot(root).render(
  <StrictMode>
    <DisplayPreferencesProvider>
      <I18nProvider>
        <App />
      </I18nProvider>
    </DisplayPreferencesProvider>
  </StrictMode>,
);
