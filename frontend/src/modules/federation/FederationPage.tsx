import { useQueryClient } from "@tanstack/react-query";
import { Link2, Plus, RefreshCcw, Trash2 } from "lucide-react";
import { type FormEvent, useState } from "react";
import { useAuth } from "../../app/auth-context";
import { useBackendConfig } from "../../shared/api/backendConfig";
import { formatDate } from "../../shared/lib/date";
import { toErrorMessage } from "../../shared/lib/errors";
import { Badge } from "../../shared/ui/Badge";
import { Button } from "../../shared/ui/Button";
import { DataTable, type DataTableColumn } from "../../shared/ui/DataTable";
import { FormField } from "../../shared/ui/FormField";
import { PageHeader } from "../../shared/ui/PageHeader";
import { useToast } from "../../shared/ui/Toast";
import { optionalText } from "../admin/api";
import {
  type FederationGuestLink,
  type FederationTrust,
  type PairingCode,
  federationQueryKeys,
  federationTrustLabel,
  useCreateFederationPairingCode,
  useCreateFederationTrust,
  useFederationGuestLinks,
  useFederationTrusts,
  useMergeFederationGuestLink,
  useRevokeFederationTrust,
} from "./api";

type TrustForm = {
  pairing_code: string;
  remote_base_url: string;
  remote_laboratory_id: string;
  tls_certificate_sha256: string;
};

const EMPTY_TRUSTS: FederationTrust[] = [];
const EMPTY_GUEST_LINKS: FederationGuestLink[] = [];

export function FederationPage() {
  const { currentUser } = useAuth();
  const { apiBaseUrl } = useBackendConfig();
  const queryClient = useQueryClient();
  const toast = useToast();
  const laboratoryId = currentUser.laboratory?.laboratory_id ?? "";
  const [pairingCode, setPairingCode] = useState<PairingCode | null>(null);
  const [trustForm, setTrustForm] = useState<TrustForm>(() => emptyTrustForm());
  const [mergeTargets, setMergeTargets] = useState<Record<string, string>>({});
  const trustsQuery = useFederationTrusts({
    enabled: Boolean(laboratoryId),
    laboratoryId,
  });
  const guestLinksQuery = useFederationGuestLinks({
    enabled: Boolean(laboratoryId),
    laboratoryId,
  });
  const createPairingCode = useCreateFederationPairingCode();
  const createTrust = useCreateFederationTrust();
  const revokeTrust = useRevokeFederationTrust();
  const mergeGuestLink = useMergeFederationGuestLink();
  const trusts = trustsQuery.data ?? EMPTY_TRUSTS;
  const guestLinks = guestLinksQuery.data ?? EMPTY_GUEST_LINKS;

  function refreshFederation() {
    queryClient.invalidateQueries({
      queryKey: federationQueryKeys.trusts(apiBaseUrl, laboratoryId),
    });
    queryClient.invalidateQueries({
      queryKey: federationQueryKeys.guestLinks(apiBaseUrl, laboratoryId),
    });
  }

  function handleCreatePairingCode() {
    if (!laboratoryId) return;
    createPairingCode.mutate(laboratoryId, {
      onError: (error) =>
        toast.error({ title: "创建配对码失败", description: toErrorMessage(error) }),
      onSuccess: (created) => {
        setPairingCode(created);
        toast.success({ title: "配对码已创建" });
      },
    });
  }

  function submitTrust(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (!laboratoryId) return;
    createTrust.mutate(
      {
        laboratoryId,
        payload: {
          pairing_code: trustForm.pairing_code.trim(),
          remote_base_url: trustForm.remote_base_url.trim(),
          remote_laboratory_id: trustForm.remote_laboratory_id.trim(),
          tls_certificate_sha256: optionalText(trustForm.tls_certificate_sha256),
        },
      },
      {
        onError: (error) =>
          toast.error({ title: "添加联邦实验室失败", description: toErrorMessage(error) }),
        onSuccess: () => {
          setTrustForm(emptyTrustForm());
          refreshFederation();
          toast.success({ title: "联邦实验室已添加" });
        },
      },
    );
  }

  function handleRevokeTrust(trustId: string) {
    if (!laboratoryId) return;
    revokeTrust.mutate(
      { laboratoryId, trustId },
      {
        onError: (error) =>
          toast.error({ title: "撤销联邦信任失败", description: toErrorMessage(error) }),
        onSuccess: () => {
          refreshFederation();
          toast.success({ title: "联邦信任已撤销" });
        },
      },
    );
  }

  function handleMergeGuestLink(link: FederationGuestLink) {
    const targetGuestUserId = mergeTargets[link.link_id]?.trim();
    if (!laboratoryId || !targetGuestUserId) return;
    mergeGuestLink.mutate(
      { laboratoryId, linkId: link.link_id, targetGuestUserId },
      {
        onError: (error) =>
          toast.error({ title: "合并 guest 失败", description: toErrorMessage(error) }),
        onSuccess: () => {
          setMergeTargets((current) => ({ ...current, [link.link_id]: "" }));
          refreshFederation();
          toast.success({ title: "guest 关联已合并" });
        },
      },
    );
  }

  const trustColumns: DataTableColumn<FederationTrust>[] = [
    {
      header: "远程实验室",
      key: "laboratory",
      render: (trust) => (
        <span className="asset-name-cell">
          <strong>{federationTrustLabel(trust)}</strong>
          <span>{trust.remote_laboratory_id}</span>
        </span>
      ),
    },
    {
      header: "远程节点",
      key: "node",
      render: (trust) => (
        <span className="asset-name-cell">
          <strong>{trust.remote_base_url}</strong>
          <span>{trust.remote_node_id}</span>
        </span>
      ),
    },
    {
      header: "状态",
      key: "status",
      render: (trust) => <Badge tone={trust.status === "active" ? "success" : "danger"}>{trust.status}</Badge>,
    },
    { header: "创建时间", key: "created", render: (trust) => formatDate(trust.created_at) },
    {
      align: "right",
      header: "操作",
      key: "actions",
      render: (trust) => (
        <Button
          disabled={trust.status !== "active" || revokeTrust.isPending}
          onClick={() => handleRevokeTrust(trust.trust_id)}
          size="icon"
          variant="danger"
          aria-label="撤销联邦信任"
        >
          <Trash2 size={15} />
        </Button>
      ),
    },
  ];

  const guestLinkColumns: DataTableColumn<FederationGuestLink>[] = [
    {
      header: "远程用户",
      key: "remote-user",
      render: (link) => (
        <span className="asset-name-cell">
          <strong>{link.remote_username}</strong>
          <span>
            {link.remote_user_type} · {link.remote_user_id}
          </span>
        </span>
      ),
    },
    {
      header: "本地 guest",
      key: "local-user",
      render: (link) => (
        <span className="asset-name-cell">
          <strong>{link.local_guest_username}</strong>
          <span>{link.local_guest_user_id}</span>
        </span>
      ),
    },
    { header: "最后访问", key: "last-seen", render: (link) => formatDate(link.last_seen_at) },
    {
      align: "right",
      header: "合并到已有 guest",
      key: "merge",
      render: (link) => (
        <div className="federation-merge-control">
          <input
            className="input"
            placeholder="本地 guest 用户 ID"
            value={mergeTargets[link.link_id] ?? ""}
            onChange={(event) =>
              setMergeTargets((current) => ({
                ...current,
                [link.link_id]: event.target.value,
              }))
            }
          />
          <Button
            disabled={!mergeTargets[link.link_id]?.trim() || mergeGuestLink.isPending}
            onClick={() => handleMergeGuestLink(link)}
          >
            合并
          </Button>
        </div>
      ),
    },
  ];

  return (
    <main className="page">
      <PageHeader
        kicker="联邦"
        title="联邦实验室"
        description="管理当前实验室与其他部署节点实验室之间的配对和信任关系。"
        actions={
          <Button
            disabled={!laboratoryId || trustsQuery.isFetching || guestLinksQuery.isFetching}
            onClick={refreshFederation}
          >
            <RefreshCcw size={15} />
            刷新
          </Button>
        }
      />

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">一次性配对码</h2>
            <p className="panel-description">把配对码交给对端实验室管理员完成首次信任建立。</p>
          </div>
          <Button
            disabled={!laboratoryId || createPairingCode.isPending}
            onClick={handleCreatePairingCode}
            variant="primary"
          >
            <Link2 size={15} />
            生成配对码
          </Button>
        </div>
        {pairingCode ? (
          <div className="panel-body federation-code-grid">
            <ProfileField label="配对码" value={pairingCode.pairing_code} />
            <ProfileField label="过期时间" value={formatDate(pairingCode.expires_at)} />
            <ProfileField label="本地节点" value={pairingCode.local_node_id} />
            <ProfileField label="本地地址" value={pairingCode.local_base_url} />
          </div>
        ) : null}
      </section>

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">添加远程联邦实验室</h2>
            <p className="panel-description">生产环境请使用 HTTPS；私有证书或 IP 证书建议填写 TLS SHA-256 指纹。</p>
          </div>
        </div>
        <form className="panel-body form-grid" onSubmit={submitTrust}>
          <div className="form-grid form-grid-2">
            <FormField label="远程节点地址">
              <input
                className="input"
                placeholder="https://10.0.0.12:8000"
                value={trustForm.remote_base_url}
                onChange={(event) =>
                  setTrustForm((current) => ({
                    ...current,
                    remote_base_url: event.target.value,
                  }))
                }
              />
            </FormField>
            <FormField label="远程实验室 ID">
              <input
                className="input"
                value={trustForm.remote_laboratory_id}
                onChange={(event) =>
                  setTrustForm((current) => ({
                    ...current,
                    remote_laboratory_id: event.target.value,
                  }))
                }
              />
            </FormField>
            <FormField label="配对码">
              <input
                className="input"
                value={trustForm.pairing_code}
                onChange={(event) =>
                  setTrustForm((current) => ({
                    ...current,
                    pairing_code: event.target.value,
                  }))
                }
              />
            </FormField>
            <FormField label="TLS SHA-256 指纹" hint="可选。输入 64 位十六进制指纹。">
              <input
                className="input"
                value={trustForm.tls_certificate_sha256}
                onChange={(event) =>
                  setTrustForm((current) => ({
                    ...current,
                    tls_certificate_sha256: event.target.value,
                  }))
                }
              />
            </FormField>
          </div>
          <div className="toolbar-group">
            <Button disabled={!canSubmitTrust(trustForm) || createTrust.isPending} type="submit" variant="primary">
              <Plus size={15} />
              添加联邦实验室
            </Button>
          </div>
        </form>
      </section>

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">已配对联邦实验室</h2>
            <p className="panel-description">侧边栏底部会显示 active 状态的远程实验室。</p>
          </div>
        </div>
        <DataTable
          columns={trustColumns}
          emptyDescription="当前实验室还没有添加远程联邦实验室。"
          getRowKey={(trust) => trust.trust_id}
          items={trusts}
          loading={trustsQuery.isLoading}
        />
      </section>

      <section className="panel">
        <div className="panel-header">
          <div>
            <h2 className="panel-title">远程 guest 关联</h2>
            <p className="panel-description">远程用户首次访问后会自动创建本地 shadow guest，可合并到已有本地 guest。</p>
          </div>
        </div>
        <DataTable
          columns={guestLinkColumns}
          emptyDescription="还没有远程用户访问过当前实验室。"
          getRowKey={(link) => link.link_id}
          items={guestLinks}
          loading={guestLinksQuery.isLoading}
        />
      </section>
    </main>
  );
}

function ProfileField({ label, value }: { label: string; value: string }) {
  return (
    <div className="profile-field">
      <p className="profile-field-label">{label}</p>
      <p className="profile-field-value">{value}</p>
    </div>
  );
}

function canSubmitTrust(form: TrustForm) {
  return Boolean(
    form.remote_base_url.trim() &&
    form.remote_laboratory_id.trim() &&
    form.pairing_code.trim(),
  );
}

function emptyTrustForm(): TrustForm {
  return {
    pairing_code: "",
    remote_base_url: "",
    remote_laboratory_id: "",
    tls_certificate_sha256: "",
  };
}
