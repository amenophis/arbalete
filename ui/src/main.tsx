import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { revealOnHover } from "./bridge";
import { Overlay } from "./Overlay";
import "./index.css";

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root element");

// The same page serves the sidebar, the sidebar revealed above the page with #reveal, and the
// overlay above the page with #overlay.
if (location.hash === "#reveal") revealOnHover(document.documentElement);
createRoot(root).render(
  <StrictMode>{location.hash === "#overlay" ? <Overlay /> : <App />}</StrictMode>,
);
