import { LayoutDashboard, List, FileText } from 'lucide-react';
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

type PulseKind = 'error' | 'warning' | 'idle';

function PulseDot({ kind }: { kind: PulseKind }) {
  return <span className={`source-pulse-dot ${kind}`} />;
}

function SidebarSection({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="source-section">
      <div className="source-section-title">{title}</div>
      {children}
    </div>
  );
}

function NavItem({
  label, active, onClick, icon,
}: {
  label: string; active: boolean; onClick: () => void; icon: React.ReactNode;
}) {
  return (
    <div
      className={`source-item${active ? ' selected' : ''}`}
      onClick={onClick}
      role="button"
      tabIndex={0}
      onKeyDown={e => e.key === 'Enter' && onClick()}
    >
      <span style={{ color: 'var(--fg-muted)', flexShrink: 0, display: 'flex' }}>{icon}</span>
      <span className="source-item-name">{label}</span>
    </div>
  );
}

function SourceItem({
  name, typeBadge, kind, hasErrors, active, onClick, count,
}: {
  name: string;
  typeBadge: string;
  kind: PulseKind;
  hasErrors: boolean;
  active: boolean;
  onClick: () => void;
  count?: string;
}) {
  return (
    <div
      className={`source-item${hasErrors ? ' has-errors' : ''}${active ? ' selected' : ''}`}
      onClick={onClick}
      role="button"
      tabIndex={0}
      onKeyDown={e => e.key === 'Enter' && onClick()}
    >
      <PulseDot kind={kind} />
      <span className="source-item-name">{name}</span>
      <span className="source-item-badge">{typeBadge}</span>
      {count != null && (
        <span className="source-item-count" title="Error count">{count}</span>
      )}
    </div>
  );
}

export function SourceMatrix({
  status, docker, httpErrors, watched,
  selectedSource, onSelectSource,
  currentView, triageService,
  onNavigateTriage, onNavigateOverview, onNavigateRaw,
}: Props) {
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

  const showWatched = (watched?.watched_files?.length ?? 0) > 0;

  return (
    <div className="source-matrix custom-scrollbar">
      {/* VIEWS */}
      <SidebarSection title="Views">
        <NavItem
          label="Overview"
          active={currentView === 'overview'}
          onClick={onNavigateOverview}
          icon={<LayoutDashboard size={14} />}
        />
        <NavItem
          label="Triage"
          active={currentView === 'triage'}
          onClick={() => onNavigateTriage(triageService ?? 'terminal')}
          icon={<List size={14} />}
        />
        <NavItem
          label="Raw Logs"
          active={currentView === 'raw'}
          onClick={onNavigateRaw}
          icon={<FileText size={14} />}
        />
      </SidebarSection>

      <div className="source-section-divider" />

      {/* SOURCES */}
      <SidebarSection title="Sources">
        {/* Terminal */}
        <SourceItem
          name="vscode_bridge"
          typeBadge="terminal"
          kind={hasPanic ? 'error' : 'idle'}
          hasErrors={hasPanic}
          active={selectedSource === 'terminal' || (currentView === 'triage' && triageService === 'terminal')}
          onClick={() => { onSelectSource('terminal'); onNavigateTriage('terminal'); }}
          count={hasPanic ? 'errors' : `${status?.buffer_lines ?? 0} lines`}
        />

        {/* Docker containers */}
        {!docker?.docker_available || docker.containers.length === 0 ? (
          <SourceItem
            name={docker?.docker_available ? 'no containers' : 'docker offline'}
            typeBadge="docker"
            kind="idle"
            hasErrors={false}
            active={false}
            onClick={() => {}}
          />
        ) : (
          docker.containers.map(cid => {
            const errCount = containerErrors[cid] ?? 0;
            const short = cid.length > 14 ? cid.slice(0, 14) : cid;
            const src = `docker:${cid}`;
            const kind: PulseKind = errCount > 0 ? 'error' : 'idle';
            return (
              <SourceItem
                key={cid}
                name={short}
                typeBadge="docker"
                kind={kind}
                hasErrors={errCount > 0}
                active={!!(selectedSource === src || (currentView === 'triage' && triageService === 'docker' && selectedSource?.includes(cid)))}
                onClick={() => { onSelectSource(src); onNavigateTriage('docker'); }}
                count={errCount > 0 ? `${errCount} err` : undefined}
              />
            );
          })
        )}

        {/* HTTP Proxy */}
        <SourceItem
          name={`proxy :${httpErrors?.proxy_port ?? 8769}`}
          typeBadge="network"
          kind={http5xx > 0 ? 'error' : http4xx > 0 ? 'warning' : 'idle'}
          hasErrors={http5xx > 0}
          active={selectedSource === 'http' || (currentView === 'triage' && triageService === 'http')}
          onClick={() => { onSelectSource('http'); onNavigateTriage('http'); }}
          count={httpTotal > 0 ? `${httpTotal} err` : undefined}
        />
      </SidebarSection>

      {/* FILES */}
      {showWatched && (
        <>
          <div className="source-section-divider" />
          <SidebarSection title="Files">
            {watched?.watched_files?.map(f => {
              const name = f.split(/[/\\]/).pop() ?? f;
              return (
                <div
                  key={f}
                  className={`source-item${selectedSource === `file:${f}` ? ' selected' : ''}`}
                  onClick={() => onSelectSource(selectedSource === `file:${f}` ? null : `file:${f}`)}
                  role="button"
                  tabIndex={0}
                  onKeyDown={e => e.key === 'Enter' && onSelectSource(selectedSource === `file:${f}` ? null : `file:${f}`)}
                >
                  <PulseDot kind="idle" />
                  <span className="source-item-name">{name}</span>
                  <span className="source-item-badge">file</span>
                </div>
              );
            })}
          </SidebarSection>
        </>
      )}
    </div>
  );
}
