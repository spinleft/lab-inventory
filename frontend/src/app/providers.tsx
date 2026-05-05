import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { App as AntApp, ConfigProvider } from "antd";
import zhCN from "antd/locale/zh_CN";
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
          mutations: {
            retry: false,
          },
        },
      }),
  );

  return (
    <ConfigProvider
      locale={zhCN}
      theme={{
        token: {
          colorPrimary: "#1677ff",
          colorBgLayout: "#f5f7fb",
          colorTextBase: "#101828",
          borderRadius: 8,
          borderRadiusLG: 8,
          controlHeight: 40,
          controlHeightLG: 44,
          fontFamily:
            'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
        },
      }}
    >
      <AntApp>
        <BackendConfigProvider>
          <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
        </BackendConfigProvider>
      </AntApp>
    </ConfigProvider>
  );
}
