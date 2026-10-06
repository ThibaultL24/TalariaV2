// web/src/components/agora/agora-modal.tsx
import type { ReactNode } from "react";
import { useI18n } from "@/lib/i18n";

interface AgoraModalProps {
  title: string;
  open: boolean;
  onClose: () => void;
  children: ReactNode;
}

export function AgoraModal({ title, open, onClose, children }: AgoraModalProps) {
  const { t } = useI18n();
  if (!open) return null;
  return (
    <div className="agora-modal-backdrop" role="presentation" onClick={onClose}>
      <div
        className="agora-modal"
        role="dialog"
        aria-modal="true"
        aria-label={title}
        onClick={(event) => event.stopPropagation()}
      >
        <div className="agora-modal__header">
          <h3>{title}</h3>
          <button type="button" onClick={onClose}>
            {t.close}
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}
