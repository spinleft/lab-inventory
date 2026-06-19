import * as Dialog from "@radix-ui/react-dialog";
import { Command } from "cmdk";
import { Search } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useNavigate } from "react-router-dom";
import { useAuth } from "./auth-context";
import { moduleCommands } from "./modules";

type CommandMenuProps = {
  onOpenChange: (open: boolean) => void;
  open: boolean;
};

export function CommandMenu({ onOpenChange, open }: CommandMenuProps) {
  const navigate = useNavigate();
  const { currentUser } = useAuth();
  const commands = useMemo(
    () =>
      moduleCommands.filter(
        (command) => !command.canAccess || command.canAccess(currentUser),
      ),
    [currentUser],
  );

  useEffect(() => {
    function handleKeyDown(event: KeyboardEvent) {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        onOpenChange(!open);
      }
    }
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [onOpenChange, open]);

  return (
    <Dialog.Root open={open} onOpenChange={onOpenChange}>
      <Dialog.Portal>
        <Dialog.Overlay className="command-overlay" />
        <Dialog.Content className="command-dialog">
          <Command loop>
            <div style={{ position: "relative" }}>
              <Search
                aria-hidden="true"
                size={16}
                style={{
                  color: "var(--text-subtle)",
                  left: 14,
                  position: "absolute",
                  top: 16,
                }}
              />
              <Command.Input
                className="command-input"
                placeholder="搜索页面、操作或设置..."
                style={{ paddingLeft: 40 }}
              />
            </div>
            <Command.List className="command-list">
              <Command.Empty className="command-empty">没有匹配的操作</Command.Empty>
              {commands.map((command) => {
                const Icon = command.icon;
                return (
                  <Command.Item
                    className="command-item"
                    key={`${command.path}-${command.title}`}
                    keywords={command.keywords}
                    value={command.title}
                    onSelect={() => {
                      navigate(command.path);
                      onOpenChange(false);
                    }}
                  >
                    <Icon size={15} aria-hidden="true" />
                    {command.title}
                  </Command.Item>
                );
              })}
            </Command.List>
          </Command>
        </Dialog.Content>
      </Dialog.Portal>
    </Dialog.Root>
  );
}

export function useCommandMenuState() {
  return useState(false);
}
