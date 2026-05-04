import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { type PropsWithChildren, useState } from "react";
import { BackendConfigProvider } from "../shared/api/backendConfig";

export function AppProviders({ children }: PropsWithChildren) {
  const [queryClient] = useState(
    () =>
      new QueryClient({
        defaultOptions: {
          queries: {
            retry: false,
            refetchOnWindowFocus: false,
          },
        },
      }),
  );

  return (
    <BackendConfigProvider>
      <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
    </BackendConfigProvider>
  );
}
