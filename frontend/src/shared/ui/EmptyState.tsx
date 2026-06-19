import { Inbox } from "lucide-react";

export function EmptyState({ description, title }: { description: string; title: string }) {
  return (
    <div className="empty-state">
      <Inbox size={24} aria-hidden="true" />
      <div>
        <h3>{title}</h3>
        <p>{description}</p>
      </div>
    </div>
  );
}
