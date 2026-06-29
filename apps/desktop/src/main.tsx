import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import {isMac, isWindows} from "./lib/platform";
import {applyThemeMode, bindSystemThemeListener, getInitialThemeMode} from "./lib/theme";
import {useAppStore} from "./lib/use-app-store";
import "./styles.css";

applyThemeMode(getInitialThemeMode());
bindSystemThemeListener(() => useAppStore.getState().themeMode);
if (isMac) document.documentElement.classList.add("is-mac");
if (isWindows) document.documentElement.classList.add("is-win");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
