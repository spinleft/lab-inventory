import {
  createContext,
  type PropsWithChildren,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
} from "react";
import { type Laboratory, useLaboratories } from "../modules/admin/api";
import {
  canManageLaboratoryAssets,
  canSelectAssetQueryLaboratory,
} from "../modules/auth/permissions";
import {
  type FederationTrust,
  federationTrustLabel,
  useFederationTrusts,
} from "../modules/federation/api";
import {
  type LaboratoryDataScope,
  laboratoryScopeId,
  laboratoryScopeKey,
  localLaboratoryScope,
  parseLaboratoryScopeValue,
  remoteLaboratoryScope,
} from "../modules/federation/scope";
import { useAuth } from "./auth-context";

const LABORATORY_SELECTION_STORAGE_KEY = "labInventory.selectedLaboratoryId";
const LABORATORY_SCOPE_STORAGE_KEY = "labInventory.selectedLaboratoryScope";
const EMPTY_LABORATORIES: Laboratory[] = [];
const EMPTY_TRUSTS: FederationTrust[] = [];

type LaboratorySelectionContextValue = {
  canManageSelectedLaboratoryAssets: boolean;
  canSelectLaboratory: boolean;
  federationTrusts: FederationTrust[];
  federationTrustsLoading: boolean;
  isRemoteLaboratory: boolean;
  laboratories: Laboratory[];
  laboratoriesLoading: boolean;
  selectedDataScope: LaboratoryDataScope;
  selectedLaboratoryId: string;
  selectedLaboratoryName: string;
  selectedScopeValue: string;
  setSelectedLaboratoryId: (laboratoryId: string) => void;
  setSelectedRemoteLaboratory: (remoteNodeId: string, remoteLaboratoryId: string) => void;
  setSelectedScopeValue: (scopeValue: string) => void;
};

const LaboratorySelectionContext =
  createContext<LaboratorySelectionContextValue | null>(null);

export function LaboratorySelectionProvider({ children }: PropsWithChildren) {
  const { currentUser } = useAuth();
  const ownLaboratoryId = currentUser.laboratory?.laboratory_id ?? "";
  const canSelectLaboratory = canSelectAssetQueryLaboratory(currentUser);
  const canUseFederation =
    (currentUser.user_type.name === "lab_admin" || currentUser.user_type.name === "user") &&
    Boolean(ownLaboratoryId);
  const laboratoriesQuery = useLaboratories({ enabled: canSelectLaboratory });
  const federationTrustsQuery = useFederationTrusts({
    enabled: canUseFederation,
    laboratoryId: ownLaboratoryId,
  });
  const laboratories = laboratoriesQuery.data ?? EMPTY_LABORATORIES;
  const federationTrusts = useMemo(
    () => (federationTrustsQuery.data ?? EMPTY_TRUSTS).filter((trust) => trust.status === "active"),
    [federationTrustsQuery.data],
  );
  const [selectedScopeValueState, setSelectedScopeValueState] = useState(() => {
    const storedScopeValue = readStoredLaboratoryScopeValue();
    if (storedScopeValue) {
      return storedScopeValue;
    }
    const storedLaboratoryId = canSelectLaboratory ? readStoredLaboratoryId() : "";
    const initialLaboratoryId = storedLaboratoryId || ownLaboratoryId;
    return initialLaboratoryId ? laboratoryScopeKey(localLaboratoryScope(initialLaboratoryId)) : "";
  });

  const trustByScopeKey = useMemo(() => {
    const next = new Map<string, FederationTrust>();
    for (const trust of federationTrusts) {
      next.set(
        laboratoryScopeKey(
          remoteLaboratoryScope(trust.remote_node_id, trust.remote_laboratory_id),
        ),
        trust,
      );
    }
    return next;
  }, [federationTrusts]);

  const setSelectedScopeValue = useCallback((scopeValue: string) => {
    setSelectedScopeValueState(scopeValue);
    if (!scopeValue) {
      window.localStorage.removeItem(LABORATORY_SCOPE_STORAGE_KEY);
      window.localStorage.removeItem(LABORATORY_SELECTION_STORAGE_KEY);
      return;
    }

    window.localStorage.setItem(LABORATORY_SCOPE_STORAGE_KEY, scopeValue);
    const parsed = parseLaboratoryScopeValue(scopeValue);
    if (parsed?.kind === "local") {
      window.localStorage.setItem(LABORATORY_SELECTION_STORAGE_KEY, parsed.laboratoryId);
    } else {
      window.localStorage.removeItem(LABORATORY_SELECTION_STORAGE_KEY);
    }
  }, []);

  const setSelectedLaboratoryId = useCallback(
    (laboratoryId: string) => {
      setSelectedScopeValue(
        laboratoryId ? laboratoryScopeKey(localLaboratoryScope(laboratoryId)) : "",
      );
    },
    [setSelectedScopeValue],
  );

  const setSelectedRemoteLaboratory = useCallback(
    (remoteNodeId: string, remoteLaboratoryId: string) => {
      setSelectedScopeValue(
        laboratoryScopeKey(remoteLaboratoryScope(remoteNodeId, remoteLaboratoryId)),
      );
    },
    [setSelectedScopeValue],
  );

  useEffect(() => {
    if (!canSelectLaboratory) {
      setSelectedLaboratoryId(ownLaboratoryId);
      return;
    }

    if (laboratories.length === 0) {
      if (laboratoriesQuery.isLoading || laboratoriesQuery.isFetching) {
        return;
      }
      if (selectedScopeValueState) {
        setSelectedLaboratoryId("");
      }
      return;
    }

    const parsedScope = parseLaboratoryScopeValue(selectedScopeValueState);
    if (parsedScope?.kind === "remote") {
      if (
        canUseFederation &&
        (federationTrustsQuery.isLoading || federationTrustsQuery.isFetching)
      ) {
        return;
      }
      if (trustByScopeKey.has(laboratoryScopeKey(parsedScope))) {
        return;
      }
    }

    if (
      parsedScope?.kind === "local" &&
      laboratories.some((lab) => lab.laboratory_id === parsedScope.laboratoryId)
    ) {
      return;
    }

    const fallbackLaboratoryId =
      ownLaboratoryId && laboratories.some((lab) => lab.laboratory_id === ownLaboratoryId)
        ? ownLaboratoryId
        : laboratories[0].laboratory_id;
    setSelectedLaboratoryId(fallbackLaboratoryId);
  }, [
    canSelectLaboratory,
    canUseFederation,
    federationTrustsQuery.isFetching,
    federationTrustsQuery.isLoading,
    laboratories,
    laboratoriesQuery.isFetching,
    laboratoriesQuery.isLoading,
    ownLaboratoryId,
    selectedScopeValueState,
    setSelectedLaboratoryId,
    trustByScopeKey,
  ]);

  const selectedDataScope = useMemo<LaboratoryDataScope>(() => {
    const parsedScope = parseLaboratoryScopeValue(selectedScopeValueState);
    if (parsedScope?.kind === "remote") {
      if (trustByScopeKey.has(laboratoryScopeKey(parsedScope))) {
        return parsedScope;
      }
      return localLaboratoryScope(ownLaboratoryId || laboratories[0]?.laboratory_id || "");
    }
    if (parsedScope?.kind === "local") {
      return parsedScope;
    }
    return localLaboratoryScope(ownLaboratoryId || laboratories[0]?.laboratory_id || "");
  }, [laboratories, ownLaboratoryId, selectedScopeValueState, trustByScopeKey]);

  const selectedLaboratoryId = laboratoryScopeId(selectedDataScope);
  const selectedScopeValue = laboratoryScopeKey(selectedDataScope);

  const selectedLaboratoryName = useMemo(() => {
    if (selectedDataScope.kind === "remote") {
      const trust = trustByScopeKey.get(laboratoryScopeKey(selectedDataScope));
      return trust ? federationTrustLabel(trust) : selectedDataScope.remoteLaboratoryId;
    }
    const selectedLaboratory = laboratories.find(
      (laboratory) => laboratory.laboratory_id === selectedLaboratoryId,
    );
    return selectedLaboratory?.name ?? currentUser.laboratory?.name ?? "";
  }, [
    currentUser.laboratory?.name,
    laboratories,
    selectedDataScope,
    selectedLaboratoryId,
    trustByScopeKey,
  ]);

  const value = useMemo<LaboratorySelectionContextValue>(
    () => ({
      canManageSelectedLaboratoryAssets:
        selectedDataScope.kind === "local" &&
        canManageLaboratoryAssets(currentUser, selectedLaboratoryId),
      canSelectLaboratory,
      federationTrusts,
      federationTrustsLoading:
        federationTrustsQuery.isLoading || federationTrustsQuery.isFetching,
      isRemoteLaboratory: selectedDataScope.kind === "remote",
      laboratories,
      laboratoriesLoading: laboratoriesQuery.isLoading || laboratoriesQuery.isFetching,
      selectedDataScope,
      selectedLaboratoryId,
      selectedLaboratoryName,
      selectedScopeValue,
      setSelectedLaboratoryId,
      setSelectedRemoteLaboratory,
      setSelectedScopeValue,
    }),
    [
      canSelectLaboratory,
      currentUser,
      federationTrusts,
      federationTrustsQuery.isFetching,
      federationTrustsQuery.isLoading,
      laboratories,
      laboratoriesQuery.isFetching,
      laboratoriesQuery.isLoading,
      selectedDataScope,
      selectedLaboratoryId,
      selectedLaboratoryName,
      selectedScopeValue,
      setSelectedLaboratoryId,
      setSelectedRemoteLaboratory,
      setSelectedScopeValue,
    ],
  );

  return (
    <LaboratorySelectionContext.Provider value={value}>
      {children}
    </LaboratorySelectionContext.Provider>
  );
}

export function useLaboratorySelection() {
  const context = useContext(LaboratorySelectionContext);
  if (!context) {
    throw new Error(
      "useLaboratorySelection must be used inside LaboratorySelectionProvider.",
    );
  }
  return context;
}

function readStoredLaboratoryId() {
  return window.localStorage.getItem(LABORATORY_SELECTION_STORAGE_KEY) ?? "";
}

function readStoredLaboratoryScopeValue() {
  return window.localStorage.getItem(LABORATORY_SCOPE_STORAGE_KEY) ?? "";
}
