import { useMutation } from "@tanstack/react-query";
import { z } from "zod";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";

export const registrationCodeSchema = z.object({
  expires_at: z.string(),
  laboratory_id: z.string().uuid(),
  registration_code: z.string(),
  registration_code_id: z.string().uuid(),
});

export type RegistrationCode = z.infer<typeof registrationCodeSchema>;

export type GuestRegistration = {
  email: string;
  password: string;
  phone_number: string;
  registration_code: string;
  username: string;
};

/**
 * Mints a registration code for the caller's laboratory.
 *
 * There is only ever one live code per laboratory: the API revokes the
 * previous one, so calling this again invalidates a code already handed out.
 */
export function useCreateGuestRegistrationCode() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return registrationCodeSchema.parse(
        await client.post("/local/guest-registration-codes"),
      );
    },
  });
}

/**
 * Redeems a code into a guest account.
 *
 * Open to anyone holding a code — no session — and rate limited by the API, so
 * callers have to be ready for a 429. Registering does not sign the guest in.
 */
export function useRegisterGuest() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (registration: GuestRegistration) => {
      const client = createApiClient(apiBaseUrl);
      await client.post("/auth/guest-registration", registration);
    },
  });
}
