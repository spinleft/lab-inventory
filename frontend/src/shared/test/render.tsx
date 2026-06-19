import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, type RenderOptions } from "@testing-library/react";
import { type ReactElement } from "react";
import { BrowserRouter } from "react-router-dom";
import { BackendConfigProvider } from "../api/backendConfig";
import { ThemeProvider } from "../theme/ThemeProvider";
import { ToastProvider } from "../ui/Toast";

export function renderApp(ui: ReactElement, options?: RenderOptions) {
  const queryClient = new QueryClient({
    defaultOptions: {
      mutations: { retry: false },
      queries: { retry: false },
    },
  });

  return render(
    <ThemeProvider>
      <ToastProvider>
        <BackendConfigProvider>
          <QueryClientProvider client={queryClient}>
            <BrowserRouter>{ui}</BrowserRouter>
          </QueryClientProvider>
        </BackendConfigProvider>
      </ToastProvider>
    </ThemeProvider>,
    options,
  );
}
