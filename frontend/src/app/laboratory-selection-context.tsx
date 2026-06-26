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
import { useAuth } from "./auth-context";

const LABORATORY_SELECTION_STORAGE_KEY = "labInventory.selectedLaboratoryId";
const EMPTY_LABORATORIES: Laboratory[] = [];

type LaboratorySelectionContextValue = {
  canManageSelectedLaboratoryAssets: boolean;
  canSelectLaboratory: boolean;
  laboratories: Laboratory[];
  laboratoriesLoading: boolean;
  selectedLaboratoryId: string;
  selectedLaboratoryName: string;
  setSelectedLaboratoryId: (laboratoryId: string) => void;
};

const LaboratorySelectionContext =
  createContext<LaboratorySelectionContextValue | null>(null);

export function LaboratorySelectionProvider({ children }: PropsWithChildren) {
  const { currentUser } = useAuth();
  const ownLaboratoryId = currentUser.laboratory?.laboratory_id ?? "";
  const canSelectLaboratory = canSelectAssetQueryLaboratory(currentUser);
  const laboratoriesQuery = useLaboratories({ enabled: canSelectLaboratory });
  const laboratories = laboratoriesQuery.data ?? EMPTY_LABORATORIES;
  const [selectedLaboratoryId, setSelectedLaboratoryIdState] = useState(() => {
    if (!canSelectLaboratory) {
      return ownLaboratoryId;
    }
    return readStoredLaboratoryId() || ownLaboratoryId;
  });

  const setSelectedLaboratoryId = useCallback((laboratoryId: string) => {
    setSelectedLaboratoryIdState(laboratoryId);
    if (laboratoryId) {
      window.localStorage.setItem(LABORATORY_SELECTION_STORAGE_KEY, laboratoryId);
    } else {
      window.localStorage.removeItem(LABORATORY_SELECTION_STORAGE_KEY);
    }
  }, []);

  useEffect(() => {
    if (!canSelectLaboratory) {
      setSelectedLaboratoryIdState(ownLaboratoryId);
      return;
    }

    if (laboratories.length === 0) {
      if (laboratoriesQuery.isLoading || laboratoriesQuery.isFetching) {
        return;
      }
      if (selectedLaboratoryId) {
        setSelectedLaboratoryId("");
      }
      return;
    }

    if (!laboratories.some((lab) => lab.laboratory_id === selectedLaboratoryId)) {
      const fallbackLaboratoryId =
        ownLaboratoryId &&
        laboratories.some((lab) => lab.laboratory_id === ownLaboratoryId)
          ? ownLaboratoryId
          : laboratories[0].laboratory_id;
      setSelectedLaboratoryId(fallbackLaboratoryId);
    }
  }, [
    canSelectLaboratory,
    laboratories,
    laboratoriesQuery.isFetching,
    laboratoriesQuery.isLoading,
    ownLaboratoryId,
    selectedLaboratoryId,
    setSelectedLaboratoryId,
  ]);

  const selectedLaboratoryName = useMemo(() => {
    const selectedLaboratory = laboratories.find(
      (laboratory) => laboratory.laboratory_id === selectedLaboratoryId,
    );
    return selectedLaboratory?.name ?? currentUser.laboratory?.name ?? "";
  }, [currentUser.laboratory?.name, laboratories, selectedLaboratoryId]);

  const value = useMemo<LaboratorySelectionContextValue>(
    () => ({
      canManageSelectedLaboratoryAssets: canManageLaboratoryAssets(
        currentUser,
        selectedLaboratoryId,
      ),
      canSelectLaboratory,
      laboratories,
      laboratoriesLoading: laboratoriesQuery.isLoading || laboratoriesQuery.isFetching,
      selectedLaboratoryId,
      selectedLaboratoryName,
      setSelectedLaboratoryId,
    }),
    [
      canSelectLaboratory,
      currentUser,
      laboratories,
      laboratoriesQuery.isFetching,
      laboratoriesQuery.isLoading,
      selectedLaboratoryId,
      selectedLaboratoryName,
      setSelectedLaboratoryId,
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
