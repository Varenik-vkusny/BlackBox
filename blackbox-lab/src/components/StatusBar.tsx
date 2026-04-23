import type { BBStatus } from '../types';

interface Props {
  status: BBStatus | null;
  daemonOnline: boolean;
}

export function StatusBar({}: Props) {
  return (
    <div className="status-bar">
      <div className="status-bar-brand">
        <span className="status-bar-name">BlackBox</span>
      </div>
    </div>
  );
}
