export const queryKeys = {
  auth: {
    me: (apiBaseUrl: string) => ["auth", apiBaseUrl, "me"] as const,
  },
  assets: {
    list: (apiBaseUrl: string, params: Record<string, unknown>) =>
      ["assets", apiBaseUrl, params] as const,
  },
  inventory: {
    list: (apiBaseUrl: string, params: Record<string, unknown>) =>
      ["inventory", apiBaseUrl, params] as const,
  },
  admin: {
    laboratories: (apiBaseUrl: string) =>
      ["admin", apiBaseUrl, "laboratories"] as const,
    users: (apiBaseUrl: string) => ["admin", apiBaseUrl, "users"] as const,
  },
  alerts: {
    stock: (apiBaseUrl: string) => ["alerts", apiBaseUrl, "stock"] as const,
    borrowRequests: (apiBaseUrl: string) =>
      ["alerts", apiBaseUrl, "borrow-requests"] as const,
    maintenance: (apiBaseUrl: string) =>
      ["alerts", apiBaseUrl, "maintenance"] as const,
  },
};
