import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import * as Tooltip from "@radix-ui/react-tooltip";
import { type PropsWithChildren, useState } from "react";
import { BackendConfigProvider } from "../shared/api/backendConfig";
import { ThemeProvider } from "../shared/theme/ThemeProvider";
import { ToastProvider } from "../shared/ui/Toast";

export function AppProviders({ children }: PropsWithChildren) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            retry: false,
            refetchOnWindowFocus: false,
          },
          mutations: {
            retry: false,
          },
        },
      }),
  );

  return (
    <ThemeProvider>
      <ToastProvider>
        <BackendConfigProvider>
          <QueryClientProvider client={queryClient}>
            <Tooltip.Provider delayDuration={250}>{children}</Tooltip.Provider>
          </QueryClientProvider>
        </BackendConfigProvider>
      </ToastProvider>
    </ThemeProvider>
  );
}
