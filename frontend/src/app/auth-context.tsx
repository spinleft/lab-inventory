import { createContext, type PropsWithChildren, useContext } from "react";
import { type CurrentUser } from "../modules/auth/types";

type AuthContextValue = {
  currentUser: CurrentUser;
};

const AuthContext = createContext<AuthContextValue | null>(null);

export function AuthProvider({
  children,
  currentUser,
}: PropsWithChildren<{ currentUser: CurrentUser }>) {
  return <AuthContext.Provider value={{ currentUser }}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  const context = useContext(AuthContext);
  if (!context) {
    throw new Error("useAuth must be used inside AuthProvider.");
  }
  return context;
}
