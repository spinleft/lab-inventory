import * as Dialog from "@radix-ui/react-dialog";
import { ServerCog, X } from "lucide-react";
import { type ReactNode, useState } from "react";
import { IconButton } from "../../shared/ui/IconButton";
import { ServerSettingsForm } from "./ServerSettingsForm";

type ServerSettingsDialogProps = {
  trigger: ReactNode;
};

export function ServerSettingsDialog({ trigger }: ServerSettingsDialogProps) {
  const [open, setOpen] = useState(false);

  return (
    <Dialog.Root open={open} onOpenChange={setOpen}>
      <Dialog.Trigger asChild>{trigger}</Dialog.Trigger>
      <Dialog.Portal>
        <Dialog.Overlay className="dialog-overlay" />
        <Dialog.Content className="dialog-content">
          <div className="dialog-header">
            <div className="cluster">
              <ServerCog aria-hidden="true" size={20} />
              <Dialog.Title className="dialog-title">服务器设置</Dialog.Title>
            </div>
            <Dialog.Close asChild>
              <IconButton label="关闭">
                <X size={18} />
              </IconButton>
            </Dialog.Close>
          </div>
          <Dialog.Description className="dialog-description">
            设置当前客户端连接的后端 API 地址。
          </Dialog.Description>

          <div className="stack dialog-body">
            <ServerSettingsForm onSaved={() => setOpen(false)} />
          </div>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}
