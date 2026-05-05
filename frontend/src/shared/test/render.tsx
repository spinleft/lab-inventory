import { render } from "@testing-library/react";
import { MemoryRouter } from "react-router-dom";
import { App } from "../../app/App";
import { AppProviders } from "../../app/providers";

export function renderRoute(initialEntries = ["/"]) {
  return render(
    <MemoryRouter initialEntries={initialEntries}>
      <AppProviders>
        <App />
      </AppProviders>
    </MemoryRouter>,
  );
}
