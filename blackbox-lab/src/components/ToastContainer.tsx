import type { Toast } from '../hooks/useToast';

interface Props {
  toasts: Toast[];
  onDismiss: (id: string) => void;
}

export function ToastContainer({ toasts, onDismiss }: Props) {
  if (toasts.length === 0) return null;

  return (
    <div className="toast-container">
      {toasts.map(t => (
        <div
          key={t.id}
          className={`toast toast--${t.type}`}
          onClick={() => onDismiss(t.id)}
          role="status"
          aria-live="polite"
        >
          <span className="toast-message">{t.message}</span>
          <button className="toast-close" aria-label="Dismiss">✕</button>
        </div>
      ))}
    </div>
  );
}
