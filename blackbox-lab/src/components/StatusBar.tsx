import { Activity, GitBranch, AlertTriangle, Box } from 'lucide-react';
import type { BBStatus, DockerResponse, HttpErrorsResponse } from '../types';

interface Props {
  status: BBStatus | null;
  docker: DockerResponse | null;
  httpErrors: HttpErrorsResponse | null;
  daemonOnline: boolean;
}



export function StatusBar({
  status, docker, httpErrors, daemonOnline,
}: Props) {
  const httpErrorCount = httpErrors?.events.length ?? 0;
  const dockerOffline = docker && !docker.docker_available;
  const dockerErrors = docker?.events.filter(e => e.level === 'ERROR' || e.level === 'FATAL').length ?? 0;
  const hasErrors = (status?.has_recent_errors ?? false) || dockerErrors > 0 || httpErrorCount > 0;

  return (
    <div className="status-bar">
      {/* Brand */}
      <div className="status-bar-brand">
        <div className="status-bar-brand-icon">
          <Box size={12} strokeWidth={2.5} />
        </div>
        BlackBox
      </div>

      <div className="status-bar-sep" />

      {/* Status pills */}
      <div className="status-bar-pills">
        {/* Daemon health */}
        <span className={`status-pill ${daemonOnline ? (hasErrors ? 'error' : 'online') : 'offline'}`}>
          <Activity size={9} strokeWidth={2.5} />
          {daemonOnline ? (hasErrors ? 'errors' : 'nominal') : 'offline'}
        </span>

        {/* Git branch */}
        {status?.git_branch && (
          <span className="status-pill neutral">
            <GitBranch size={9} strokeWidth={2.5} />
            {status.git_branch}
            {status.git_dirty_files > 0 && (
              <span style={{ color: 'var(--accent-orange)' }}>+{status.git_dirty_files}</span>
            )}
          </span>
        )}

        {/* Docker status */}
        {docker && (
          <span className={`status-pill ${dockerOffline ? 'offline' : dockerErrors > 0 ? 'warn' : 'neutral'}`}>
            <Box size={9} strokeWidth={2.5} />
            docker
            {dockerOffline ? ' (offline)' : dockerErrors > 0 ? ` · ${dockerErrors} err` : ` · ${docker.containers.length}`}
          </span>
        )}

        {/* HTTP errors */}
        {httpErrorCount > 0 && (
          <span className="status-pill warn">
            <AlertTriangle size={9} strokeWidth={2.5} />
            {httpErrorCount} http err
          </span>
        )}
      </div>


    </div>
  );
}
