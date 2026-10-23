import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { Overlay } from "./Overlay";
import "./index.css";

const root = document.getElementById("root");
if (!root) throw new Error("index.html has no #root element");

// The same page serves the sidebar and, with #overlay, the overlay above the page.
createRoot(root).render(
  <StrictMode>{location.hash === "#overlay" ? <Overlay /> : <App />}</StrictMode>,
);
