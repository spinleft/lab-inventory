import * as AlertDialog from "@radix-ui/react-alert-dialog";
import { type ReactNode } from "react";
import { Button } from "./Button";

type ConfirmDialogProps = {
  cancelLabel?: string;
  confirmLabel?: string;
  description: string;
  disabled?: boolean;
  onConfirm: () => void;
  title: string;
  trigger: ReactNode;
};

export function ConfirmDialog({
  cancelLabel = "取消",
  confirmLabel = "确认",
  description,
  disabled,
  onConfirm,
  title,
  trigger,
}: ConfirmDialogProps) {
  return (
    <AlertDialog.Root>
      <AlertDialog.Trigger asChild disabled={disabled}>
        {trigger}
      </AlertDialog.Trigger>
      <AlertDialog.Portal>
        <AlertDialog.Overlay className="dialog-overlay" />
        <AlertDialog.Content className="dialog-content">
          <div className="dialog-header">
            <div>
              <AlertDialog.Title className="dialog-title">{title}</AlertDialog.Title>
              <AlertDialog.Description className="dialog-description">
                {description}
              </AlertDialog.Description>
            </div>
          </div>
          <div className="dialog-footer">
            <AlertDialog.Cancel asChild>
              <Button>{cancelLabel}</Button>
            </AlertDialog.Cancel>
            <AlertDialog.Action asChild>
              <Button variant="danger" onClick={onConfirm}>
                {confirmLabel}
              </Button>
            </AlertDialog.Action>
          </div>
        </AlertDialog.Content>
      </AlertDialog.Portal>
    </AlertDialog.Root>
  );
}
