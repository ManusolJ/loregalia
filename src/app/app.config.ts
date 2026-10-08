import type { ApplicationConfig } from "@angular/core";

import { provideRouter } from "@angular/router";
import { provideBrowserGlobalErrorListeners } from "@angular/core";

import { routes } from "./app.routes";

export const appConfig: ApplicationConfig = {
  providers: [provideBrowserGlobalErrorListeners(), provideRouter(routes)],
};
