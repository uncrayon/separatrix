import React from "react";
import ReactDOM from "react-dom/client";
import { MathfieldElement } from "mathlive";
// MathLive fonts and static-markup styles, bundled locally so the app works offline.
import "mathlive/fonts.css";
import "mathlive/static.css";
import "./styles/tokens.css";
import "./styles/app.css";
import App from "./App";

// The fonts come from the CSS imports above; stop MathLive fetching its own copy
// (and its keypress sounds) from a path that does not exist in the bundle.
MathfieldElement.fontsDirectory = null;
MathfieldElement.soundsDirectory = null;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
