import type { ComponentType } from "react";
import App from "./App";
import { PrivacyPolicy } from "./legal/PrivacyPolicy";
import { TermsOfService } from "./legal/TermsOfService";

export type Route = { path: string; title: string; description: string; Page: ComponentType };

/** Every page on the site. scripts/prerender.mjs writes one static HTML file per entry. */
export const ROUTES: Route[] = [
  {
    path: "/",
    title: "Permafrost · SolidWorks + Google Drive file sharing for FRC teams",
    description:
      "Share SolidWorks files over Google Drive. Permafrost adds check-out locks and atomic pushes; Permafrost Lite keeps it to simple upload and download. Windows and Mac.",
    Page: App,
  },
  {
    path: "/privacy-policy",
    title: "Privacy Policy · Permafrost",
    description: "What the Permafrost app and website do with your data, including the Google data the app accesses.",
    Page: PrivacyPolicy,
  },
  {
    path: "/terms-of-service",
    title: "Terms of Service · Permafrost",
    description: "The terms for using the Permafrost desktop app and website.",
    Page: TermsOfService,
  },
];

export function routeFor(pathname: string): Route {
  const path = pathname.replace(/\/+$/, "") || "/";
  return ROUTES.find((r) => r.path === path) ?? ROUTES[0];
}
