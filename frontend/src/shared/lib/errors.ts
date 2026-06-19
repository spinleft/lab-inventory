export function toErrorMessage(caught: unknown, fallback = "操作失败，请稍后重试。") {
  if (caught instanceof Error && caught.message) {
    return caught.message;
  }
  return fallback;
}
