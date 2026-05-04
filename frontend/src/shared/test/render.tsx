import { render } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { AppProviders } from "../../app/providers";
import { routes } from "../../app/routes";

export function renderRoute(initialEntries: string[] = ["/"]) {
  const router = createMemoryRouter(routes, { initialEntries });
  const result = render(
    <AppProviders>
      <RouterProvider router={router} />
    </AppProviders>,
  );
  return { router, ...result };
}
