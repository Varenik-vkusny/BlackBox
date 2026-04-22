import { Terminal, Box, Globe, File, LayoutDashboard, List } from 'lucide-react';
import type { BBStatus, DockerResponse, HttpErrorsResponse, WatchedFilesResponse } from '../types';
import type { DashboardView } from '../App';

interface Props {
  status: BBStatus | null;
  docker: DockerResponse | null;
  httpErrors: HttpErrorsResponse | null;
  watched: WatchedFilesResponse | null;
  selectedSource: string | null;
  onSelectSource: (src: string | null) => void;
  currentView: DashboardView;
  triageService: string | null;
  onNavigateTriage: (service: string) => void;
  onNavigateOverview: () => void;
  onNavigateRaw: () => void;
}

type PulseKind = 'active' | 'error' | 'warning' | 'idle';

function PulseDot({ kind }: { kind: PulseKind }) {
  return <span className={`source-pulse-dot ${kind}`} />;
}

// Orange for 1-9 errors; red for ≥10 or critical (panic)
function errorSeverity(count: number, isCritical = false): PulseKind {
  if (isCritical || count >= 10) return 'error';
  if (count > 0) return 'warning';
  return 'active';
}

function errorLabel(count: number): string {
  if (count === 0) return '';
  return `${count} error${count !== 1 ? 's' : ''}`;
}

function NavItem({
  label, icon, active, onClick,
}: { label: string; icon: React.ReactNode; active: boolean; onClick: () => void }) {
  return (
    <div
      className={`source-item${active ? ' selected' : ''}`}
      onClick={onClick}
      role="button"
      tabIndex={0}
      onKeyDown={e => e.key === 'Enter' && onClick()}
      style={{ gap: '0.5rem' }}
    >
      <span style={{ color: active ? 'var(--accent-cyan)' : 'var(--text-muted)', flexShrink: 0 }}>{icon}</span>
      <span className="source-item-name" style={{ fontFamily: 'Inter, system-ui, sans-serif', fontSize: '0.7rem' }}>
        {label}
      </span>
    </div>
  );
}

export function SourceMatrix({
  status, docker, httpErrors, watched,
  selectedSource, onSelectSource,
  currentView, triageService,
  onNavigateTriage, onNavigateOverview, onNavigateRaw,
}: Props) {
  const toggle = (src: string) => onSelectSource(selectedSource === src ? null : src);

  const containerErrors: Record<string, number> = {};
  docker?.events?.forEach(e => {
    const lvl = e.level?.toLowerCase();
    if (lvl === 'error' || lvl === 'fatal' || lvl === 'critical') {
      const id = e.source.container_id;
      containerErrors[id] = (containerErrors[id] ?? 0) + 1;
    }
  });

  const http4xx = httpErrors?.events?.filter(e => e.status < 500).length ?? 0;
  const http5xx = httpErrors?.events?.filter(e => e.status >= 500).length ?? 0;
  const httpTotal = http4xx + http5xx;

  const hasPanic = status?.has_recent_errors ?? false;
  const terminalErrorCount = 0; // buffer_lines is total, not errors; use has_recent_errors as signal
  const terminalSeverity = errorSeverity(terminalErrorCount, hasPanic);

  const showWatched = (watched?.watched_files?.length ?? 0) > 0;

  return (
    <div className="source-matrix custom-scrollbar">

      {/* ── View Navigation ── */}
      <div className="source-section">
        <div className="source-section-title">
          <LayoutDashboard size={10} /> Views
        </div>
        <NavItem
          label="Overview"
          icon={<LayoutDashboard size={12} />}
          active={currentView === 'overview'}
          onClick={onNavigateOverview}
        />
        <NavItem
          label="Raw Logs"
          icon={<List size={12} />}
          active={currentView === 'raw'}
          onClick={onNavigateRaw}
        />
      </div>

      <div className="source-section-divider" />

      {/* ── Terminals ── */}
      <div className="source-section">
        <div className="source-section-title">
          <Terminal size={10} /> Terminal
        </div>

        <div
          className={`source-item${hasPanic ? ' has-errors' : ''}${selectedSource === 'terminal' || (currentView === 'triage' && triageService === 'terminal') ? ' selected' : ''}`}
          onClick={() => { onSelectSource('terminal'); onNavigateTriage('terminal'); }}
          role="button"
          tabIndex={0}
          onKeyDown={e => e.key === 'Enter' && onNavigateTriage('terminal')}
        >
          <PulseDot kind={terminalSeverity} />
          <span className="source-item-name">vscode_bridge</span>
          {status && (
            <span className="source-item-count" title="Total buffered lines">
              {hasPanic ? 'errors' : `${status.buffer_lines} lines`}
            </span>
          )}
        </div>
      </div>

      <div className="source-section-divider" />

      {/* ── Docker ── */}
      <div className="source-section">
        <div className="source-section-title">
          <Box size={10} /> Docker
          {!docker?.docker_available && (
            <span style={{ fontSize: '0.55rem', color: 'var(--text-muted)', marginLeft: '0.25rem' }}>(offline)</span>
          )}
        </div>

        {!docker?.docker_available || docker.containers.length === 0 ? (
          <div className="source-item" style={{ cursor: 'default', opacity: 0.45 }}>
            <PulseDot kind="idle" />
            <span className="source-item-name" style={{ color: 'var(--text-muted)' }}>
              {!docker?.docker_available ? 'not connected' : 'no containers'}
            </span>
          </div>
        ) : (
          docker.containers.map(cid => {
            const errCount = containerErrors[cid] ?? 0;
            const short = cid.length > 14 ? cid.slice(0, 14) : cid;
            const src = `docker:${cid}`;
            const severity = errorSeverity(errCount);
            const isSelected = selectedSource === src || (currentView === 'triage' && triageService === 'docker' && selectedSource?.includes(cid));
            return (
              <div
                key={cid}
                className={`source-item${errCount > 0 ? (severity === 'error' ? ' has-errors' : '') : ''}${isSelected ? ' selected' : ''}`}
                onClick={() => { onSelectSource(src); onNavigateTriage('docker'); }}
                role="button"
                tabIndex={0}
                onKeyDown={e => e.key === 'Enter' && onSelectSource(src)}
              >
                <PulseDot kind={errCount > 0 ? severity : 'active'} />
                <span className="source-item-name">{short}</span>
                {errCount > 0 && (
                  <span
                    className="source-item-count"
                    style={{ color: severity === 'error' ? 'var(--accent-red)' : 'var(--accent-orange)' }}
                  >
                    {errorLabel(errCount)}
                  </span>
                )}
              </div>
            );
          })
        )}
      </div>

      <div className="source-section-divider" />

      {/* ── HTTP Proxy ── */}
      <div className="source-section">
        <div className="source-section-title">
          <Globe size={10} /> Network
        </div>

        <div
          className={`source-item${http5xx > 0 ? ' has-errors' : ''}${selectedSource === 'http' || (currentView === 'triage' && triageService === 'http') ? ' selected' : ''}`}
          onClick={() => { onSelectSource('http'); onNavigateTriage('http'); }}
          role="button"
          tabIndex={0}
          onKeyDown={e => e.key === 'Enter' && onNavigateTriage('http')}
        >
          <PulseDot kind={http5xx > 0 ? 'error' : http4xx > 0 ? 'warning' : 'idle'} />
          <span className="source-item-name">
            proxy :{httpErrors?.proxy_port ?? 8769}
          </span>
          {httpTotal > 0 && (
            <span
              className="source-item-count"
              style={{ color: http5xx > 0 ? 'var(--accent-red)' : 'var(--accent-orange)' }}
            >
              {errorLabel(httpTotal)}
            </span>
          )}
        </div>

        {/* 4xx / 5xx breakdown — only when there are errors */}
        {(http4xx > 0 || http5xx > 0) && (
          <div style={{ paddingLeft: '1.75rem', display: 'flex', gap: '0.5rem', paddingBottom: '0.25rem' }}>
            {http4xx > 0 && (
              <span style={{ fontSize: '0.6rem', color: 'var(--accent-orange)', fontFamily: 'JetBrains Mono, monospace' }}>
                {http4xx}× 4xx
              </span>
            )}
            {http5xx > 0 && (
              <span style={{ fontSize: '0.6rem', color: 'var(--accent-red)', fontFamily: 'JetBrains Mono, monospace' }}>
                {http5xx}× 5xx
              </span>
            )}
          </div>
        )}
      </div>

      {/* ── Watched Files — only when present ── */}
      {showWatched && (
        <>
          <div className="source-section-divider" />
          <div className="source-section">
            <div className="source-section-title">
              <File size={10} /> Watched Files
            </div>
            {watched?.watched_files?.map(f => (
              <div
                key={f}
                className={`source-item${selectedSource === `file:${f}` ? ' selected' : ''}`}
                onClick={() => toggle(`file:${f}`)}
                role="button"
                tabIndex={0}
                onKeyDown={e => e.key === 'Enter' && toggle(`file:${f}`)}
              >
                <PulseDot kind="active" />
                <span className="source-item-name">{f.split(/[/\\]/).pop()}</span>
              </div>
            ))}
          </div>
        </>
      )}

      {/* ── Git info ── */}
      {status?.git_branch && (
        <>
          <div className="source-section-divider" />
          <div className="source-section" style={{ paddingBottom: '0.5rem' }}>
            <div style={{
              padding: '0.4rem 0.6rem',
              fontSize: '0.62rem',
              fontFamily: 'JetBrains Mono, monospace',
              color: 'var(--text-muted)',
              display: 'flex',
              alignItems: 'center',
              gap: '0.35rem',
            }}>
              <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
                <line x1="6" y1="3" x2="6" y2="15" /><circle cx="18" cy="6" r="3" /><circle cx="6" cy="18" r="3" />
                <path d="M18 9a9 9 0 0 1-9 9" />
              </svg>
              {status.git_branch}
              {status.git_dirty_files > 0 && (
                <span style={{ color: 'var(--accent-orange)', marginLeft: '0.25rem' }}>
                  · {status.git_dirty_files} modified
                </span>
              )}
            </div>
          </div>
        </>
      )}
    </div>
  );
}
