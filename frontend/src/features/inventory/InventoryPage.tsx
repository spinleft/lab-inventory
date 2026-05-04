import { Search } from "lucide-react";
import { useState } from "react";
import { Button } from "../../shared/ui/Button";
import { NativeSelect } from "../../shared/ui/NativeSelect";
import { TextInput } from "../../shared/ui/TextInput";
import { useInventoryItems } from "./api";

const PAGE_SIZE = 20;

export function InventoryPage() {
  const [query, setQuery] = useState("");
  const [status, setStatus] = useState("");
  const [trackingMode, setTrackingMode] = useState("");
  const [borrowable, setBorrowable] = useState("");
  const [offset, setOffset] = useState(0);
  const inventory = useInventoryItems({
    q: query,
    status,
    tracking_mode: trackingMode,
    is_cross_lab_borrowable:
      borrowable === "" ? undefined : borrowable === "true",
    limit: PAGE_SIZE,
    offset,
  });

  return (
    <>
      <div className="page-header">
        <div>
          <h1>库存</h1>
          <p>查看设备序列件和材料批次数量，敏感字段由后端权限过滤。</p>
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
              placeholder="搜索资产、序列号、批次"
              aria-label="搜索库存"
            />
          </label>
          <NativeSelect
            value={status}
            onChange={(event) => {
              setOffset(0);
              setStatus(event.target.value);
            }}
            aria-label="库存状态"
          >
            <option value="">全部状态</option>
            <option value="idle">闲置</option>
            <option value="in_use">使用中</option>
            <option value="borrowed">借出</option>
            <option value="faulty">故障</option>
            <option value="maintenance">维修中</option>
            <option value="available">可用</option>
            <option value="unavailable">不可用</option>
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
          <NativeSelect
            value={borrowable}
            onChange={(event) => {
              setOffset(0);
              setBorrowable(event.target.value);
            }}
            aria-label="可借用性"
          >
            <option value="">全部可借用性</option>
            <option value="true">可跨实验室借用</option>
            <option value="false">不可跨实验室借用</option>
          </NativeSelect>
        </div>

        {inventory.isLoading ? <p className="muted">正在加载库存...</p> : null}
        {inventory.isError ? (
          <div className="alert">{inventory.error.message}</div>
        ) : null}
        {inventory.data ? (
          <>
            <div className="table-wrap">
              <table className="data-table">
                <thead>
                  <tr>
                    <th>资产</th>
                    <th>实验室</th>
                    <th>识别信息</th>
                    <th>数量</th>
                    <th>位置</th>
                    <th>状态</th>
                    <th>可借用</th>
                  </tr>
                </thead>
                <tbody>
                  {inventory.data.items.map((item) => (
                    <tr key={item.inventory_item_id}>
                      <td>
                        <strong>{item.asset_name}</strong>
                        <p className="muted small">{item.asset_model ?? "-"}</p>
                      </td>
                      <td>{item.laboratory_name}</td>
                      <td>
                        {item.serial_number ?? item.batch_number ?? (
                          <span className="muted">未公开</span>
                        )}
                      </td>
                      <td>
                        {item.quantity_available} / {item.quantity_on_hand}{" "}
                        {item.unit_code}
                      </td>
                      <td>{item.location_name ?? "-"}</td>
                      <td>
                        <span className="badge">{item.status}</span>
                      </td>
                      <td>{item.is_cross_lab_borrowable ? "是" : "否"}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {inventory.data.items.length === 0 ? (
              <p className="muted table-empty">暂无库存。</p>
            ) : null}
            <div className="pagination">
              <span className="muted small">
                {inventory.data.total === 0
                  ? "0"
                  : inventory.data.offset + 1}
                -{Math.min(inventory.data.offset + inventory.data.limit, inventory.data.total)} /{" "}
                {inventory.data.total}
              </span>
              <div className="cluster">
                <Button
                  type="button"
                  variant="secondary"
                  onClick={() => setOffset(Math.max(0, offset - PAGE_SIZE))}
                  disabled={offset === 0}
                >
                  上一页
                </Button>
                <Button
                  type="button"
                  variant="secondary"
                  onClick={() => setOffset(offset + PAGE_SIZE)}
                  disabled={inventory.data.offset + inventory.data.limit >= inventory.data.total}
                >
                  下一页
                </Button>
              </div>
            </div>
          </>
        ) : null}
      </section>
    </>
  );
}
