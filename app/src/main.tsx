import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { QuickAdd } from "./components/QuickAdd";
import { api } from "./lib/api";
import type { Screen } from "./lib/store";
import "./styles.css";

// The quick add window loads the same page with #quick; #screen=<name> opens on that screen, with the
// preview flags in api.ts after it (for screenshots).
const quick = window.location.hash === "#quick";
if (/[#&]still(&|$)/.test(window.location.hash)) document.documentElement.classList.add("still");
const initialScreen = window.location.hash.match(/^#screen=([a-z]+)(?:&|$)/)?.[1] as Screen | undefined;

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{quick ? <QuickAdd api={api} /> : <App api={api} initialScreen={initialScreen} />}</React.StrictMode>,
);
