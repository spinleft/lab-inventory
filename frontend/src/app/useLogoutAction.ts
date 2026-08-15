import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "react-router-dom";
import { useLogout } from "../modules/auth/api";

/**
 * Signing out, from wherever the control happens to live.
 *
 * The desktop shell puts it in the sidebar's user menu and the phone shell on
 * the "更多" page; both have to clear the cache, or the next user to sign in on
 * this device sees the previous one's data before the refetch lands.
 */
export function useLogoutAction() {
  const logout = useLogout();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  return {
    isPending: logout.isPending,
    logout: () =>
      logout.mutate(undefined, {
        onSettled: () => {
          queryClient.clear();
          navigate("/login", { replace: true });
        },
      }),
  };
}
