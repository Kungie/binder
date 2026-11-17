import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import { QuickAdd } from "./components/QuickAdd";
import { api } from "./lib/api";
import "./styles.css";

// The quick add window loads the same page with #quick.
const quick = window.location.hash === "#quick";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>{quick ? <QuickAdd api={api} /> : <App api={api} />}</React.StrictMode>,
);
