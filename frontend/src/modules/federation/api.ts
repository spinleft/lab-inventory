import { useMutation, useQuery } from "@tanstack/react-query";
import { z } from "zod";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { createApiClient } from "../../shared/api/httpClient";

export const federationTrustSchema = z.object({
  created_at: z.string(),
  local_laboratory_id: z.string().uuid(),
  remote_base_url: z.string(),
  remote_laboratory_id: z.string().uuid(),
  remote_laboratory_name: z.string().nullable(),
  remote_node_id: z.string().uuid(),
  revoked_at: z.string().nullable(),
  status: z.string(),
  trust_id: z.string().uuid(),
  updated_at: z.string(),
});

export const pairingCodeSchema = z.object({
  expires_at: z.string(),
  local_base_url: z.string(),
  local_laboratory_id: z.string().uuid(),
  local_node_id: z.string().uuid(),
  pairing_code: z.string(),
  pairing_code_id: z.string().uuid(),
});

export const federationGuestLinkSchema = z.object({
  first_seen_at: z.string(),
  last_seen_at: z.string(),
  link_id: z.string().uuid(),
  local_guest_user_id: z.string().uuid(),
  local_guest_username: z.string(),
  local_laboratory_id: z.string().uuid(),
  remote_base_url: z.string(),
  remote_laboratory_id: z.string().uuid(),
  remote_node_id: z.string().uuid(),
  remote_user_id: z.string().uuid(),
  remote_user_type: z.string(),
  remote_username: z.string(),
});

const federationTrustsSchema = z.array(federationTrustSchema);
const federationGuestLinksSchema = z.array(federationGuestLinkSchema);

export type FederationTrust = z.infer<typeof federationTrustSchema>;
export type PairingCode = z.infer<typeof pairingCodeSchema>;
export type FederationGuestLink = z.infer<typeof federationGuestLinkSchema>;

export type CreateFederationTrustPayload = {
  pairing_code: string;
  remote_base_url: string;
  remote_laboratory_id: string;
  tls_certificate_sha256?: string | null;
};

export const federationQueryKeys = {
  guestLinks: (apiBaseUrl: string, laboratoryId: string) =>
    ["federation", apiBaseUrl, "guest-links", laboratoryId] as const,
  root: (apiBaseUrl: string) => ["federation", apiBaseUrl] as const,
  trusts: (apiBaseUrl: string, laboratoryId: string) =>
    ["federation", apiBaseUrl, "trusts", laboratoryId] as const,
};

export function useFederationTrusts({
  enabled = true,
  laboratoryId,
}: {
  enabled?: boolean;
  laboratoryId: string;
}) {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    enabled: enabled && Boolean(laboratoryId),
    queryKey: federationQueryKeys.trusts(apiBaseUrl, laboratoryId),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return federationTrustsSchema.parse(
        await client.get(`/laboratories/${laboratoryId}/federation/trusts`),
      );
    },
  });
}

export function useFederationGuestLinks({
  enabled = true,
  laboratoryId,
}: {
  enabled?: boolean;
  laboratoryId: string;
}) {
  const { apiBaseUrl } = useBackendConfig();

  return useQuery({
    enabled: enabled && Boolean(laboratoryId),
    queryKey: federationQueryKeys.guestLinks(apiBaseUrl, laboratoryId),
    queryFn: async () => {
      const client = createApiClient(apiBaseUrl);
      return federationGuestLinksSchema.parse(
        await client.get(`/laboratories/${laboratoryId}/federation/guest-links`),
      );
    },
  });
}

export function useCreateFederationPairingCode() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async (laboratoryId: string) => {
      const client = createApiClient(apiBaseUrl);
      return pairingCodeSchema.parse(
        await client.post(`/laboratories/${laboratoryId}/federation/pairing-codes`),
      );
    },
  });
}

export function useCreateFederationTrust() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async ({
      laboratoryId,
      payload,
    }: {
      laboratoryId: string;
      payload: CreateFederationTrustPayload;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return federationTrustSchema.parse(
        await client.post(`/laboratories/${laboratoryId}/federation/trusts`, payload),
      );
    },
  });
}

export function useRevokeFederationTrust() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async ({
      laboratoryId,
      trustId,
    }: {
      laboratoryId: string;
      trustId: string;
    }) => {
      const client = createApiClient(apiBaseUrl);
      await client.delete(`/laboratories/${laboratoryId}/federation/trusts/${trustId}`);
    },
  });
}

export function useMergeFederationGuestLink() {
  const { apiBaseUrl } = useBackendConfig();

  return useMutation({
    mutationFn: async ({
      laboratoryId,
      linkId,
      targetGuestUserId,
    }: {
      laboratoryId: string;
      linkId: string;
      targetGuestUserId: string;
    }) => {
      const client = createApiClient(apiBaseUrl);
      return federationGuestLinkSchema.parse(
        await client.post(
          `/laboratories/${laboratoryId}/federation/guest-links/${linkId}/merge`,
          { target_guest_user_id: targetGuestUserId },
        ),
      );
    },
  });
}

export function federationTrustLabel(trust: FederationTrust) {
  return trust.remote_laboratory_name?.trim() || trust.remote_laboratory_id;
}
