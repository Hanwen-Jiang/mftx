import React from "react";
import ReactDOM from "react-dom/client";

import App from "./App";
import {isMac} from "./lib/platform";
import {applyTheme, getInitialTheme} from "./lib/theme";
import "./styles.css";

applyTheme(getInitialTheme());
if (isMac) document.documentElement.classList.add("is-mac");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
