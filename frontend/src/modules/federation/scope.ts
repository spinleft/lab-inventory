export type LocalLaboratoryScope = {
  kind: "local";
  laboratoryId: string;
};

export type RemoteLaboratoryScope = {
  kind: "remote";
  remoteLaboratoryId: string;
  remoteNodeId: string;
};

export type LaboratoryDataScope = LocalLaboratoryScope | RemoteLaboratoryScope;

export function localLaboratoryScope(laboratoryId: string): LocalLaboratoryScope {
  return { kind: "local", laboratoryId };
}

export function remoteLaboratoryScope(
  remoteNodeId: string,
  remoteLaboratoryId: string,
): RemoteLaboratoryScope {
  return { kind: "remote", remoteNodeId, remoteLaboratoryId };
}

export function laboratoryScopeId(scope: LaboratoryDataScope) {
  return scope.kind === "local" ? scope.laboratoryId : scope.remoteLaboratoryId;
}

export function laboratoryScopeKey(scope: LaboratoryDataScope) {
  if (scope.kind === "local") {
    return `local:${scope.laboratoryId}`;
  }
  return `remote:${scope.remoteNodeId}:${scope.remoteLaboratoryId}`;
}

export function laboratoryScopeCacheKey(scope: LaboratoryDataScope) {
  return scope.kind === "local" ? scope.laboratoryId : laboratoryScopeKey(scope);
}

export function laboratoryDetailScopeCacheKey(scope: LaboratoryDataScope | undefined) {
  return scope?.kind === "remote" ? laboratoryScopeKey(scope) : "local";
}

export function parseLaboratoryScopeValue(value: string): LaboratoryDataScope | null {
  const parts = value.split(":");
  if (parts[0] === "local" && parts.length === 2 && parts[1]) {
    return localLaboratoryScope(parts[1]);
  }
  if (parts[0] === "remote" && parts.length === 3 && parts[1] && parts[2]) {
    return remoteLaboratoryScope(parts[1], parts[2]);
  }
  return null;
}

export function laboratoryCollectionPath(scope: LaboratoryDataScope, collection: string) {
  if (scope.kind === "local") {
    return `/laboratories/${scope.laboratoryId}/${collection}`;
  }
  return `/federation/nodes/${scope.remoteNodeId}/laboratories/${scope.remoteLaboratoryId}/${collection}`;
}

export function assetDetailPath(scope: LaboratoryDataScope | undefined, assetId: string) {
  if (scope?.kind === "remote") {
    return `/federation/nodes/${scope.remoteNodeId}/laboratories/${scope.remoteLaboratoryId}/assets/${assetId}`;
  }
  return `/assets/${assetId}`;
}

export function inventoryItemDetailPath(
  scope: LaboratoryDataScope | undefined,
  inventoryItemId: string,
) {
  if (scope?.kind === "remote") {
    return `/federation/nodes/${scope.remoteNodeId}/laboratories/${scope.remoteLaboratoryId}/inventory-items/${inventoryItemId}`;
  }
  return `/inventory-items/${inventoryItemId}`;
}
