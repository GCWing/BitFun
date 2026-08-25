import React from "react";
import ReactDOM from "react-dom/client";
import "@bitfun/theme-bitfun/default.css";
import "@bitfun/ui/styles.css";
import { App } from "./App";
import { I18nProvider } from "./i18n";
import "./styles.css";

const rootElement = document.getElementById("root");

if (!rootElement) {
  throw new Error("Design Lab root element was not found.");
}

ReactDOM.createRoot(rootElement).render(
  <React.StrictMode>
    <I18nProvider>
      <App />
    </I18nProvider>
  </React.StrictMode>,
);
