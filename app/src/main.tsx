import React from "react";
import ReactDOM from "react-dom/client";
import { MathfieldElement } from "mathlive";
import "mathlive/fonts.css";
import "mathlive/static.css";
import "./styles/ds/tokens.css";
import "./styles/ds/components.css";
import "./styles/app.css";
import "./styles/areas/shell.css";
import "./styles/areas/system.css";
import "./styles/areas/inspector.css";
import "./styles/areas/canvas.css";
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
