import type { BorrowRequestStatus } from "./api";

export function borrowRequestLabel(status: BorrowRequestStatus) {
  if (status === "pending") return "待审批";
  if (status === "approved") return "已批准";
  if (status === "cancelled") return "已撤销";
  return "已拒绝";
}

export function borrowRequestTone(status: BorrowRequestStatus) {
  if (status === "pending") return "warning" as const;
  if (status === "approved") return "success" as const;
  if (status === "cancelled") return "default" as const;
  return "danger" as const;
}
