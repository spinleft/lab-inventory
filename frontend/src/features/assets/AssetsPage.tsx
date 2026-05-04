import { Search } from "lucide-react";
import { useState } from "react";
import { Button } from "../../shared/ui/Button";
import { NativeSelect } from "../../shared/ui/NativeSelect";
import { TextInput } from "../../shared/ui/TextInput";
import { useAssets } from "./api";

const PAGE_SIZE = 20;

export function AssetsPage() {
  const [query, setQuery] = useState("");
  const [assetKind, setAssetKind] = useState("");
  const [trackingMode, setTrackingMode] = useState("");
  const [offset, setOffset] = useState(0);
  const assets = useAssets({
    q: query,
    asset_kind: assetKind,
    tracking_mode: trackingMode,
    limit: PAGE_SIZE,
    offset,
  });

  return (
    <>
      <div className="page-header">
        <div>
          <h1>资产</h1>
          <p>按实验室、类型和追踪方式查看设备与材料信息。</p>
        </div>
      </div>

      <section className="panel panel-pad">
        <div className="filters">
          <label className="search-field">
            <Search aria-hidden="true" size={18} />
            <TextInput
              value={query}
              onChange={(event) => {
                setOffset(0);
                setQuery(event.target.value);
              }}
              placeholder="搜索名称、型号、厂商"
              aria-label="搜索资产"
            />
          </label>
          <NativeSelect
            value={assetKind}
            onChange={(event) => {
              setOffset(0);
              setAssetKind(event.target.value);
            }}
            aria-label="资产类型"
          >
            <option value="">全部类型</option>
            <option value="device">设备</option>
            <option value="material">材料</option>
          </NativeSelect>
          <NativeSelect
            value={trackingMode}
            onChange={(event) => {
              setOffset(0);
              setTrackingMode(event.target.value);
            }}
            aria-label="追踪方式"
          >
            <option value="">全部追踪方式</option>
            <option value="unique">序列追踪</option>
            <option value="quantity">数量追踪</option>
          </NativeSelect>
        </div>

        {assets.isLoading ? <p className="muted">正在加载资产...</p> : null}
        {assets.isError ? <div className="alert">{assets.error.message}</div> : null}
        {assets.data ? (
          <>
            <div className="table-wrap">
              <table className="data-table">
                <thead>
                  <tr>
                    <th>名称</th>
                    <th>型号</th>
                    <th>实验室</th>
                    <th>类型</th>
                    <th>追踪</th>
                    <th>单位</th>
                    <th>状态</th>
                  </tr>
                </thead>
                <tbody>
                  {assets.data.items.map((asset) => (
                    <tr key={asset.asset_id}>
                      <td>
                        <strong>{asset.name}</strong>
                        {asset.category_name ? (
                          <p className="muted small">{asset.category_name}</p>
                        ) : null}
                      </td>
                      <td>{asset.model ?? "-"}</td>
                      <td>{asset.laboratory_name}</td>
                      <td>{labelAssetKind(asset.asset_kind)}</td>
                      <td>{labelTrackingMode(asset.tracking_mode)}</td>
                      <td>{asset.default_unit_code}</td>
                      <td>
                        <span className="badge">
                          {asset.is_archived ? "已归档" : "有效"}
                        </span>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {assets.data.items.length === 0 ? (
              <p className="muted table-empty">暂无资产。</p>
            ) : null}
            <Pagination
              limit={assets.data.limit}
              offset={assets.data.offset}
              total={assets.data.total}
              onPrevious={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
              onNext={() => setOffset(offset + PAGE_SIZE)}
            />
          </>
        ) : null}
      </section>
    </>
  );
}

function Pagination({
  limit,
  offset,
  total,
  onPrevious,
  onNext,
}: {
  limit: number;
  offset: number;
  total: number;
  onPrevious: () => void;
  onNext: () => void;
}) {
  return (
    <div className="pagination">
      <span className="muted small">
        {total === 0 ? "0" : offset + 1}-{Math.min(offset + limit, total)} / {total}
      </span>
      <div className="cluster">
        <Button
          type="button"
          variant="secondary"
          onClick={onPrevious}
          disabled={offset === 0}
        >
          上一页
        </Button>
        <Button
          type="button"
          variant="secondary"
          onClick={onNext}
          disabled={offset + limit >= total}
        >
          下一页
        </Button>
      </div>
    </div>
  );
}

function labelAssetKind(value: string) {
  return value === "device" ? "设备" : value === "material" ? "材料" : value;
}

function labelTrackingMode(value: string) {
  return value === "unique" ? "序列追踪" : value === "quantity" ? "数量追踪" : value;
}
