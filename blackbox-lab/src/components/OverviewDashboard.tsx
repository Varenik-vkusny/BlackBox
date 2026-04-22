import { useState, useRef, useEffect } from 'react';
import { Activity, GitCommit, File, ChevronRight, AlertTriangle, CheckCircle, WifiOff, Box, Globe, Terminal, Maximize2 } from 'lucide-react';
import type {
  BBStatus, CompressedResponse, DockerResponse, HttpErrorsResponse,
  PostmortemResponse, RecentCommitsResponse, WatchedFilesResponse,
} from '../types';

interface Props {
  status: BBStatus | null;
  compressed: CompressedResponse | null;
  docker: DockerResponse | null;
  httpErrors: HttpErrorsResponse | null;
  postmortem: PostmortemResponse | null;
  commits: RecentCommitsResponse | null;
  watched: WatchedFilesResponse | null;
  logs: string[];
  daemonOnline: boolean;
  onNavigateTriage: (service: string) => void;
  onNavigateRaw: () => void;
}

function timeAgo(isoOrMs: string | number): string {
  const ts = typeof isoOrMs === 'number' ? isoOrMs : Date.parse(isoOrMs);
  const diffSecs = Math.floor((Date.now() - ts) / 1000);
  if (diffSecs < 60) return `${diffSecs}s ago`;
  if (diffSecs < 3600) return `${Math.floor(diffSecs / 60)}m ago`;
  if (diffSecs < 86400) return `${Math.floor(diffSecs / 3600)}h ago`;
  return `${Math.floor(diffSecs / 86400)}d ago`;
}

function severityClass(errorCount: number, hasPanic: boolean): 'nominal' | 'warning' | 'critical' | 'offline' {
  if (hasPanic || errorCount >= 10) return 'critical';
  if (errorCount > 0) return 'warning';
  return 'nominal';
}

interface ServiceCardProps {
  name: string;
  type: string;
  icon: React.ReactNode;
  severity: 'nominal' | 'warning' | 'critical' | 'offline';
  stat: string;
  detail: string;
  onClick: () => void;
}

function ServiceCard({ name, type, icon, severity, stat, detail, onClick }: ServiceCardProps) {
  return (
    <div className={`service-card ${severity}`} onClick={onClick} role="button" tabIndex={0}
      onKeyDown={e => e.key === 'Enter' && onClick()}>
      <div className="service-card-header">
        <div className="service-card-dot" />
        <span className="service-card-name">{name}</span>
        <span className="service-card-type">{type}</span>
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem' }}>
        <span style={{ color: 'var(--text-muted)', flexShrink: 0 }}>{icon}</span>
        <span className="service-card-stat">{stat}</span>
      </div>
      <div className="service-card-detail">{detail}</div>
      <div className="service-card-cta">
        Inspect errors <ChevronRight size={10} style={{ display: 'inline', verticalAlign: 'middle' }} />
      </div>
    </div>
  );
}

function MiniHistogram({ postmortem }: { postmortem: PostmortemResponse }) {
  const timeline = postmortem.timeline;
  if (!timeline.length) return null;

  const maxErrors = Math.max(...timeline.map(b => b.error_count), 1);
  const maxLines  = Math.max(...timeline.map(b => b.line_count), 1);
  const svgH = 48;
  const barW = Math.max(2, Math.floor(320 / timeline.length) - 1);

  return (
    <div className="overview-histogram">
      <div className="overview-histogram-header">
        <span className="overview-section-label" style={{ margin: 0 }}>
          <Activity size={11} /> 30-min timeline
        </span>
        <span style={{ fontSize: '0.6rem', color: 'var(--text-muted)' }}>
          {postmortem.total_lines} lines · {postmortem.docker_events_in_window} docker events
        </span>
      </div>
      <div className="overview-histogram-body">
        <svg width="100%" height={svgH} viewBox={`0 0 ${timeline.length * (barW + 1)} ${svgH}`}
          preserveAspectRatio="none" style={{ display: 'block' }}>
          {timeline.map((bucket, i) => {
            const x = i * (barW + 1);
            const lineH = Math.max(2, Math.round((bucket.line_count / maxLines) * (svgH - 4)));
            const errH  = Math.max(bucket.error_count > 0 ? 2 : 0, Math.round((bucket.error_count / maxErrors) * (svgH - 4)));
            return (
              <g key={i}>
                <rect x={x} y={svgH - lineH} width={barW} height={lineH}
                  fill="rgba(34,211,238,0.15)" rx="1" />
                {errH > 0 && (
                  <rect x={x} y={svgH - errH} width={barW} height={errH}
                    fill="rgba(244,63,94,0.6)" rx="1" />
                )}
              </g>
            );
          })}
        </svg>
        <div style={{ display: 'flex', justifyContent: 'space-between', marginTop: '0.25rem' }}>
          <span style={{ fontSize: '0.55rem', color: 'var(--text-muted)', fontFamily: 'JetBrains Mono, monospace' }}>-30m</span>
          <span style={{ fontSize: '0.55rem', color: 'var(--text-muted)', fontFamily: 'JetBrains Mono, monospace' }}>now</span>
        </div>
        <div style={{ display: 'flex', gap: '1rem', marginTop: '0.375rem' }}>
          <span style={{ fontSize: '0.58rem', color: 'rgba(34,211,238,0.7)', display: 'flex', alignItems: 'center', gap: '0.25rem' }}>
            <span style={{ display: 'inline-block', width: 8, height: 8, background: 'rgba(34,211,238,0.4)', borderRadius: 2 }} /> log volume
          </span>
          <span style={{ fontSize: '0.58rem', color: 'rgba(244,63,94,0.8)', display: 'flex', alignItems: 'center', gap: '0.25rem' }}>
            <span style={{ display: 'inline-block', width: 8, height: 8, background: 'rgba(244,63,94,0.6)', borderRadius: 2 }} /> errors
          </span>
        </div>
      </div>
    </div>
  );
}

function levelColor(text: string): string {
  const t = text.toLowerCase();
  if (/\b(fatal|panic)\b/.test(t)) return 'var(--accent-red)';
  if (/\berror\b/.test(t)) return '#f87171';
  if (/\bwarn(ing)?\b/.test(t)) return 'var(--accent-orange)';
  return 'var(--text-secondary)';
}

interface InlineLogViewerProps {
  logs: string[];
  docker: DockerResponse | null;
  httpErrors: HttpErrorsResponse | null;
  onFullscreen: () => void;
}

interface LogEntry {
  id: string;
  text: string;
  source: string;
  level: string | null;
  timestamp_ms: number;
}

function buildEntries(
  logs: string[],
  docker: DockerResponse | null,
  httpErrors: HttpErrorsResponse | null,
): LogEntry[] {
  const entries: LogEntry[] = [];
  const now = Date.now();

  // Terminal lines — spread across last 30s as approximation
  const reversed = logs.slice().reverse();
  const step = reversed.length > 1 ? 30000 / reversed.length : 0;
  reversed.forEach((text, i) => {
    const t = text.toLowerCase();
    const level =
      /\b(fatal|panic)\b/.test(t) ? 'fatal'
      : /\berror\b/.test(t) ? 'error'
      : /\bwarn(ing)?\b/.test(t) ? 'warn'
      : null;
    entries.push({ id: `t-${i}`, text, source: 'bash', level, timestamp_ms: now - i * step });
  });

  // Docker events
  docker?.events?.forEach((ev, i) => {
    entries.push({
      id: `d-${i}`,
      text: ev.text,
      source: `docker:${ev.source.container_id.slice(0, 8)}`,
      level: ev.level?.toLowerCase() ?? null,
      timestamp_ms: ev.timestamp_ms,
    });
  });

  // HTTP errors
  httpErrors?.events?.forEach((ev, i) => {
    entries.push({
      id: `h-${i}`,
      text: `${ev.method} ${ev.url} → ${ev.status}`,
      source: `http:${ev.status}`,
      level: ev.status >= 500 ? 'error' : 'warn',
      timestamp_ms: ev.timestamp_ms,
    });
  });

  // Sort oldest-first so newest is at bottom (for auto-scroll)
  return entries.sort((a, b) => a.timestamp_ms - b.timestamp_ms);
}

const SOURCE_TAG_COLORS: Record<string, string> = {
  bash: 'rgba(34,211,238,0.25)',
  docker: 'rgba(96,165,250,0.25)',
  http: 'rgba(251,191,36,0.25)',
};

function sourceTagColor(source: string): string {
  const key = source.split(':')[0];
  return SOURCE_TAG_COLORS[key] ?? 'rgba(148,163,184,0.15)';
}

function InlineLogViewer({ logs, docker, httpErrors, onFullscreen }: InlineLogViewerProps) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

  const entries = buildEntries(logs, docker, httpErrors);
  const recent = entries.slice(-120);
  const total = entries.length;

  // Auto-scroll to bottom when new entries arrive
  useEffect(() => {
    if (autoScroll && scrollRef.current) {
      scrollRef.current.scrollTop = scrollRef.current.scrollHeight;
    }
  }, [total, autoScroll]);

  const handleScroll = () => {
    if (!scrollRef.current) return;
    const el = scrollRef.current;
    const nearBottom = el.scrollHeight - el.scrollTop - el.clientHeight < 40;
    setAutoScroll(nearBottom);
  };

  return (
    <div style={{
      border: '1px solid var(--border)',
      borderRadius: 'var(--radius-sm)',
      background: 'rgba(0,0,0,0.3)',
      display: 'flex',
      flexDirection: 'column',
      overflow: 'hidden',
      minHeight: 180,
      maxHeight: 260,
    }}>
      {/* Header */}
      <div style={{
        display: 'flex',
        alignItems: 'center',
        gap: '0.5rem',
        padding: '0.35rem 0.6rem',
        borderBottom: '1px solid var(--border)',
        background: 'rgba(0,0,0,0.2)',
        flexShrink: 0,
      }}>
        <Terminal size={10} style={{ color: 'var(--accent-cyan)', flexShrink: 0 }} />
        <span style={{ fontSize: '0.6rem', fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.12em', color: 'var(--text-muted)', flex: 1 }}>
          Live Logs
        </span>
        <span style={{ fontSize: '0.58rem', color: 'var(--text-muted)', fontFamily: "'JetBrains Mono', monospace" }}>
          {total} entries
        </span>
        <button
          onClick={onFullscreen}
          title="Open full log view"
          style={{
            background: 'none',
            border: '1px solid var(--border)',
            borderRadius: 4,
            cursor: 'pointer',
            color: 'var(--text-muted)',
            display: 'flex',
            alignItems: 'center',
            gap: '0.25rem',
            padding: '0.15rem 0.4rem',
            fontSize: '0.58rem',
            fontWeight: 600,
            letterSpacing: '0.05em',
            transition: 'color 0.15s, border-color 0.15s',
          }}
          onMouseEnter={e => {
            (e.currentTarget as HTMLButtonElement).style.color = 'var(--accent-cyan)';
            (e.currentTarget as HTMLButtonElement).style.borderColor = 'rgba(34,211,238,0.4)';
          }}
          onMouseLeave={e => {
            (e.currentTarget as HTMLButtonElement).style.color = 'var(--text-muted)';
            (e.currentTarget as HTMLButtonElement).style.borderColor = 'var(--border)';
          }}
        >
          <Maximize2 size={9} />
          Fullscreen
        </button>
      </div>

      {/* Log lines */}
      <div
        ref={scrollRef}
        onScroll={handleScroll}
        className="custom-scrollbar"
        style={{
          flex: 1,
          overflowY: 'auto',
          padding: '0.35rem 0',
          fontFamily: "'JetBrains Mono', monospace",
          fontSize: '0.62rem',
          lineHeight: 1.6,
        }}
      >
        {recent.length === 0 ? (
          <div style={{ padding: '1rem', textAlign: 'center', color: 'var(--text-muted)', fontSize: '0.65rem' }}>
            No log entries yet…
          </div>
        ) : (
          recent.map(entry => (
            <div key={entry.id} style={{
              padding: '0 0.65rem',
              display: 'flex',
              alignItems: 'baseline',
              gap: '0.4rem',
              color: levelColor(entry.text),
            }}>
              <span style={{
                flexShrink: 0,
                fontSize: '0.55rem',
                padding: '0 0.3rem',
                borderRadius: 3,
                background: sourceTagColor(entry.source),
                color: 'var(--text-muted)',
                letterSpacing: '0.04em',
              }}>
                {entry.source}
              </span>
              <span style={{ whiteSpace: 'pre-wrap', wordBreak: 'break-all' }}>
                {entry.text}
              </span>
            </div>
          ))
        )}
      </div>
    </div>
  );
}

export function OverviewDashboard({
  status, compressed, docker, httpErrors, postmortem, commits, watched, logs,
  daemonOnline, onNavigateTriage, onNavigateRaw,
}: Props) {

  if (!daemonOnline) {
    return (
      <div className="overview-root" style={{ alignItems: 'center', justifyContent: 'center' }}>
        <div className="overview-health-banner offline" style={{ maxWidth: 400 }}>
          <div className="health-indicator-dot offline" />
          <div>
            <div className="health-headline offline">Daemon Offline</div>
            <div className="health-subtext">Start the BlackBox daemon to begin monitoring.</div>
          </div>
          <WifiOff size={20} style={{ color: 'var(--text-muted)', marginLeft: 'auto' }} />
        </div>
      </div>
    );
  }

  const totalClusters = compressed?.clusters.filter(c => c.level === 'error' || c.level === 'fatal').length ?? 0;
  const totalTraces = compressed?.stack_traces.length ?? 0;
  
  // Docker errors = real docker events + log clusters tagged as docker
  const dockerEventErrors = docker?.events.filter(e => {
    const lvl = e.level?.toLowerCase();
    return lvl === 'error' || lvl === 'fatal';
  }).length ?? 0;
  const dockerClusterErrors = compressed?.clusters.filter(c => 
    (c.level === 'error' || c.level === 'fatal') && 
    c.sources?.some(s => s.startsWith('docker:'))
  ).length ?? 0;
  const dockerErrors = dockerEventErrors + dockerClusterErrors;

  const httpErrCount = httpErrors?.events.length ?? 0;
  
  // Terminal errors = clusters/traces tagged as terminal
  const terminalErrors = (compressed?.stack_traces.length ?? 0) + 
    (compressed?.clusters.filter(c => 
      (c.level === 'error' || c.level === 'fatal') && 
      (c.sources?.includes('terminal') || !c.sources || c.sources.length === 0)
    ).length ?? 0);

  const totalErrors = totalTraces + dockerErrors + httpErrCount + (totalClusters - dockerClusterErrors);
  const hasPanic = compressed?.stack_traces.some(t => t.language === 'rust') ?? (status?.has_recent_errors || false);

  // Overall system health
  const overallSeverity: 'nominal' | 'warning' | 'critical' = hasPanic || totalErrors >= 5
    ? 'critical' : totalErrors > 0 ? 'warning' : 'nominal';

  const healthLabel = overallSeverity === 'nominal'
    ? 'All Systems Nominal'
    : overallSeverity === 'warning'
    ? `${totalErrors} incident${totalErrors > 1 ? 's' : ''} detected`
    : hasPanic
    ? `Panic detected — ${totalErrors} active issue${totalErrors > 1 ? 's' : ''}`
    : `${totalErrors} critical error${totalErrors > 1 ? 's' : ''}`;

  const healthSub = overallSeverity === 'nominal'
    ? `Monitoring ${status?.buffer_lines ?? 0} log lines · ${status?.project_type ?? 'unknown'} project`
    : overallSeverity === 'warning'
    ? 'Anomalies detected in log clusters or network traffic'
    : 'Critical failure or panic detected · click a card to triage';

  // Terminal service
  const terminalSeverity = severityClass(terminalErrors, hasPanic);

  // Docker service
  const dockerAvailable = docker?.docker_available ?? false;
  const dockerSeverity: 'nominal' | 'warning' | 'critical' | 'offline' = !dockerAvailable
    ? 'offline' : severityClass(dockerErrors, false);

  // HTTP service
  const httpSeverity = severityClass(httpErrCount, false);

  return (
    <div className="overview-root custom-scrollbar">
      {/* Health banner */}
      <div className={`overview-health-banner ${overallSeverity}`}>
        <div className={`health-indicator-dot ${overallSeverity}`} />
        <div style={{ flex: 1 }}>
          <div className={`health-headline ${overallSeverity}`}>{healthLabel}</div>
          <div className="health-subtext">{healthSub}</div>
        </div>
        {overallSeverity === 'nominal'
          ? <CheckCircle size={20} style={{ color: 'var(--accent-green)', opacity: 0.7, flexShrink: 0 }} />
          : <AlertTriangle size={20} style={{ color: overallSeverity === 'critical' ? 'var(--accent-red)' : 'var(--accent-orange)', flexShrink: 0 }} />
        }
      </div>

      {/* Metric pills */}
      <div className="overview-metrics-row">
        <div className="metric-pill">
          <div className={`metric-pill-value ${totalTraces > 0 ? 'red' : ''}`}>{totalTraces}</div>
          <div className="metric-pill-label">Stack Traces</div>
        </div>
        <div className="metric-pill">
          <div className={`metric-pill-value ${totalClusters > 0 ? 'orange' : ''}`}>{totalClusters}</div>
          <div className="metric-pill-label">Log Clusters</div>
        </div>
        <div className="metric-pill">
          <div className={`metric-pill-value ${dockerErrors > 0 ? 'orange' : ''}`}>
            {dockerAvailable ? docker?.containers.length ?? 0 : '—'}
          </div>
          <div className="metric-pill-label">Containers</div>
        </div>
        <div className="metric-pill">
          <div className={`metric-pill-value ${httpErrCount > 0 ? 'red' : ''}`}>{httpErrCount}</div>
          <div className="metric-pill-label">HTTP Errors</div>
        </div>
      </div>

      {/* Service cards */}
      <div>
        <div className="overview-section-label">
          <Activity size={11} /> Services
        </div>
        <div className="service-cards-grid">
          <ServiceCard
            name="vscode_bridge"
            type="terminal"
            icon={<Terminal size={14} />}
            severity={terminalSeverity}
            stat={terminalSeverity === 'nominal'
              ? `${status?.buffer_lines ?? 0} log lines`
              : `${terminalErrors} error lines`}
            detail={hasPanic
              ? `Rust panic detected · ${totalTraces} stack trace${totalTraces !== 1 ? 's' : ''}`
              : totalClusters > 0
              ? `${totalClusters} log cluster${totalClusters !== 1 ? 's' : ''} · ${status?.buffer_lines ?? 0} total lines`
              : `${status?.buffer_lines ?? 0} lines · no errors`}
            onClick={() => onNavigateTriage('terminal')}
          />
          <ServiceCard
            name={dockerAvailable ? `${docker?.containers.length ?? 0} container${(docker?.containers.length ?? 0) !== 1 ? 's' : ''}` : 'Docker'}
            type="docker"
            icon={<Box size={14} />}
            severity={dockerSeverity}
            stat={dockerSeverity === 'offline'
              ? 'not connected'
              : dockerErrors > 0
              ? `${dockerErrors} error${dockerErrors !== 1 ? 's' : ''}`
              : 'no errors'}
            detail={dockerSeverity === 'offline'
              ? 'Docker daemon not reachable'
              : docker?.containers.join(', ') || 'No containers running'}
            onClick={() => onNavigateTriage('docker')}
          />
          <ServiceCard
            name="http-proxy"
            type="network"
            icon={<Globe size={14} />}
            severity={httpSeverity}
            stat={httpErrCount > 0
              ? `${httpErrCount} error${httpErrCount !== 1 ? 's' : ''}`
              : httpErrors ? 'no errors' : 'idle'}
            detail={httpErrCount > 0
              ? httpErrors?.events.slice(0, 2).map(e => `${e.method} ${e.status}`).join(' · ') ?? ''
              : `Proxy port ${httpErrors?.proxy_port ?? '—'} · no 4xx/5xx`}
            onClick={() => onNavigateTriage('http')}
          />
        </div>
      </div>

      {/* Histogram + commits */}
      <div className="overview-bottom-grid">
        {/* 30-min histogram */}
        <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
          {postmortem && postmortem.timeline.length > 0 && (
            <MiniHistogram postmortem={postmortem} />
          )}

          {/* Inline log viewer */}
          <InlineLogViewer logs={logs} docker={docker} httpErrors={httpErrors} onFullscreen={onNavigateRaw} />
        </div>

        {/* Recent commits */}
        <div>
          <div className="overview-section-label">
            <GitCommit size={11} /> Recent Commits
          </div>
          {commits && commits.commits.length > 0 ? (
            <div className="commit-strip">
              {commits.commits.slice(0, 5).map(c => (
                <div className="commit-row" key={c.hash}>
                  <span className="commit-hash">{c.hash.slice(0, 7)}</span>
                  <span className="commit-msg">{c.message}</span>
                  <span className="commit-meta">{timeAgo(c.timestamp_iso)}</span>
                </div>
              ))}
            </div>
          ) : (
            <div style={{ fontSize: '0.7rem', color: 'var(--text-muted)', fontFamily: 'JetBrains Mono, monospace', padding: '0.5rem 0' }}>
              No recent commits
            </div>
          )}

          {/* Watched files (only when present) */}
          {watched && watched.count > 0 && (
            <div style={{ marginTop: '0.875rem' }}>
              <div className="overview-section-label">
                <File size={11} /> Watched Files ({watched.count})
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: '0.25rem' }}>
                {watched.watched_files.slice(0, 4).map(f => (
                  <div key={f} style={{
                    fontSize: '0.65rem', fontFamily: 'JetBrains Mono, monospace',
                    color: 'var(--text-muted)', padding: '0.2rem 0',
                    overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap',
                  }}>
                    {f}
                  </div>
                ))}
                {watched.count > 4 && (
                  <div style={{ fontSize: '0.6rem', color: 'var(--text-muted)' }}>
                    +{watched.count - 4} more
                  </div>
                )}
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
