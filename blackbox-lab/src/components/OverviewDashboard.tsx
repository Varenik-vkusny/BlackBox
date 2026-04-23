import { useState } from 'react';
import { GitCommit, Activity, Maximize2 } from 'lucide-react';
import type {
  BBStatus, CompressedResponse, DockerResponse, HttpErrorsResponse,
  PostmortemResponse, RecentCommitsResponse, LogLine,
} from '../types';

interface Props {
  status: BBStatus | null;
  compressed: CompressedResponse | null;
  docker: DockerResponse | null;
  httpErrors: HttpErrorsResponse | null;
  postmortem: PostmortemResponse | null;
  commits: RecentCommitsResponse | null;
  logLines: LogLine[];
  daemonOnline: boolean;
  onNavigateTriage: (service: string) => void;
  onNavigateRaw: () => void;
}

/* ═══════════════════════════════════════════════════
   Helpers
   ═══════════════════════════════════════════════════ */

function timeAgo(isoOrMs: string | number): string {
  const ts = typeof isoOrMs === 'number' ? isoOrMs : Date.parse(isoOrMs);
  const diffSecs = Math.floor((Date.now() - ts) / 1000);
  if (diffSecs < 60) return `${diffSecs}s ago`;
  if (diffSecs < 3600) return `${Math.floor(diffSecs / 60)}m ago`;
  if (diffSecs < 86400) return `${Math.floor(diffSecs / 3600)}h ago`;
  return `${Math.floor(diffSecs / 86400)}d ago`;
}

function formatUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m`;
  if (secs < 86400) return `${Math.floor(secs / 3600)}h`;
  return `${Math.floor(secs / 86400)}d`;
}

/* ═══════════════════════════════════════════════════
   Inline Metrics
   ═══════════════════════════════════════════════════ */

function InlineMetrics({
  traces, clusters, containers, httpErr, branch, dirty, uptimeSecs,
}: {
  traces: number; clusters: number; containers: number; httpErr: number;
  branch: string | null; dirty: number; uptimeSecs: number;
}) {
  return (
    <div className="overview-metrics-inline">
      <span className={traces > 0 ? 'metric-err' : 'metric-zero'}>{traces} trace{traces !== 1 ? 's' : ''}</span>
      <span className="metric-sep">·</span>
      <span className={clusters > 0 ? 'metric-err' : 'metric-zero'}>{clusters} cluster{clusters !== 1 ? 's' : ''}</span>
      <span className="metric-sep">·</span>
      <span className="metric-zero">{containers} container{containers !== 1 ? 's' : ''}</span>
      <span className="metric-sep">·</span>
      <span className={httpErr > 0 ? 'metric-err' : 'metric-zero'}>{httpErr} http error{httpErr !== 1 ? 's' : ''}</span>
      <span className="metric-sep">·</span>
      {branch ? (
        <>{branch}{dirty > 0 && <span className="metric-err"> +{dirty}</span>}</>
      ) : (
        <span className="metric-zero">no git</span>
      )}
      <span className="metric-sep">·</span>
      <span className="metric-zero">{formatUptime(uptimeSecs)} ago</span>
    </div>
  );
}

/* ═══════════════════════════════════════════════════
   Focus Block
   ═══════════════════════════════════════════════════ */

interface FocusBlockProps {
  trace: { language: string; error_message: string; captured_at_ms: number };
  moreCount: number;
  onInspect: () => void;
  onDismiss: () => void;
}

function FocusBlock({ trace, moreCount, onInspect, onDismiss }: FocusBlockProps) {
  return (
    <div className="focus-block">
      <div className="focus-block-title">Active Issue</div>
      <div className="focus-block-body">{trace.error_message}</div>
      <div className="focus-block-meta">
        <span>{trace.language}</span>
        <span>·</span>
        <span>vscode_bridge</span>
        <span>·</span>
        <span>{timeAgo(trace.captured_at_ms)}</span>
        {moreCount > 0 && (
          <>
            <span>·</span>
            <button className="focus-block-more" onClick={onInspect}>+{moreCount} more active →</button>
          </>
        )}
      </div>
      <div className="focus-block-actions">
        <button className="focus-block-btn" onClick={onInspect}>Inspect trace</button>
        <button className="focus-block-btn" onClick={onDismiss}>Dismiss</button>
      </div>
    </div>
  );
}

/* ═══════════════════════════════════════════════════
   Service List
   ═══════════════════════════════════════════════════ */

interface ServiceRowProps {
  dot: 'error' | 'ok';
  name: string;
  type: string;
  status: string;
  statusSeverity?: 'error' | 'warn' | null;
  cta?: string;
  onClick: () => void;
}

function ServiceRow({ dot, name, type, status, statusSeverity, cta, onClick }: ServiceRowProps) {
  return (
    <div className="service-list-row" onClick={onClick} role="button" tabIndex={0} onKeyDown={e => e.key === 'Enter' && onClick()}>
      <span className={`service-list-dot ${dot}`}>{dot === 'error' ? '●' : '○'}</span>
      <span className="service-list-name">{name}</span>
      <span className="service-list-type">{type}</span>
      <span className={`service-list-status${statusSeverity ? ` ${statusSeverity}` : ''}`}>{status}</span>
      {cta && (
        <button className="service-list-cta" onClick={e => { e.stopPropagation(); }}>{cta}</button>
      )}
    </div>
  );
}

/* ═══════════════════════════════════════════════════
   Mini Histogram (reused, cleaned)
   ═══════════════════════════════════════════════════ */

function MiniHistogram({ postmortem }: { postmortem: PostmortemResponse }) {
  const timeline = postmortem.timeline;
  if (!timeline.length) return null;

  const maxErrors = Math.max(...timeline.map(b => b.error_count), 1);
  const maxLines  = Math.max(...timeline.map(b => b.line_count), 1);
  const svgH = 60;
  const barW = Math.max(2, Math.floor(280 / timeline.length) - 1);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '8px' }}>
      <div className="overview-section-label" style={{ marginBottom: 0 }}>
        <Activity size={11} /> 60-min timeline
      </div>
      <svg width="100%" height={svgH} viewBox={`0 0 ${timeline.length * (barW + 1)} ${svgH}`}
        preserveAspectRatio="none" style={{ display: 'block' }}>
        {timeline.map((bucket, i) => {
          const x = i * (barW + 1);
          const lineH = Math.max(2, Math.round((bucket.line_count / maxLines) * (svgH - 4)));
          const errH  = Math.max(bucket.error_count > 0 ? 2 : 0, Math.round((bucket.error_count / maxErrors) * (svgH - 4)));
          return (
            <g key={i}>
              <rect x={x} y={svgH - lineH} width={barW} height={lineH} fill="rgba(91,96,102,0.2)" rx="1" />
              {errH > 0 && <rect x={x} y={svgH - errH} width={barW} height={errH} fill="rgba(229,72,77,0.6)" rx="1" />}
            </g>
          );
        })}
      </svg>
      <div style={{ display: 'flex', justifyContent: 'space-between' }}>
        <span style={{ fontSize: '11px', color: 'var(--fg-muted)', fontFamily: "'Geist Mono', monospace" }}>-60m</span>
        <span style={{ fontSize: '11px', color: 'var(--fg-muted)', fontFamily: "'Geist Mono', monospace" }}>now</span>
      </div>
    </div>
  );
}

/* ═══════════════════════════════════════════════════
   Inline Log Viewer (without fullscreen)
   ═══════════════════════════════════════════════════ */

function timeLabel(ms: number): string {
  return new Date(ms).toTimeString().slice(0, 8);
}

function InlineLogViewer({ logLines, onFullscreen }: { logLines: LogLine[]; onFullscreen: () => void }) {
  const recent = logLines.slice(-80);
  return (
    <div style={{
      border: '1px solid var(--border-subtle)',
      borderRadius: 'var(--radius-sm)',
      background: 'var(--bg-base)',
      display: 'flex',
      flexDirection: 'column',
      overflow: 'hidden',
      minHeight: 120,
      maxHeight: 220,
    }}>
      <div style={{
        display: 'flex', alignItems: 'center', gap: '8px',
        padding: '6px 10px', borderBottom: '1px solid var(--border-subtle)',
        background: 'var(--bg-raised)', flexShrink: 0,
      }}>
        <span style={{ fontSize: '11px', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.08em', color: 'var(--fg-muted)', flex: 1 }}>Live Logs</span>
        <span style={{ fontSize: '11px', color: 'var(--fg-muted)', fontFamily: "'Geist Mono', monospace" }}>{logLines.length} entries</span>
        <button onClick={onFullscreen} title="Open full log view" style={{
          background: 'none', border: '1px solid var(--border-subtle)', borderRadius: '4px',
          cursor: 'pointer', color: 'var(--fg-muted)', display: 'flex', alignItems: 'center',
          gap: '4px', padding: '2px 6px', fontSize: '11px', fontWeight: 600,
          transition: 'color 0.15s, border-color 0.15s',
        }}
          onMouseEnter={e => { (e.currentTarget as HTMLButtonElement).style.color = 'var(--fg-secondary)'; (e.currentTarget as HTMLButtonElement).style.borderColor = 'var(--border-default)'; }}
          onMouseLeave={e => { (e.currentTarget as HTMLButtonElement).style.color = 'var(--fg-muted)'; (e.currentTarget as HTMLButtonElement).style.borderColor = 'var(--border-subtle)'; }}>
          <Maximize2 size={10} />
        </button>
      </div>
      <div className="custom-scrollbar" style={{ flex: 1, overflowY: 'auto', fontFamily: "'Geist Mono', monospace", fontSize: '12px', lineHeight: 1.6 }}>
        {recent.length === 0
          ? <div style={{ padding: '12px', textAlign: 'center', color: 'var(--fg-muted)', fontSize: '12px' }}>—</div>
          : recent.map((line, i) => (
            <div key={i} className="inline-log-row">
              <span className="inline-log-time">{timeLabel(line.timestamp_ms)}</span>
              {line.source_terminal && line.source_terminal !== '' && (
                <span className="inline-log-source">{line.source_terminal}</span>
              )}
              <span className="inline-log-text">{line.text}</span>
            </div>
          ))
        }
      </div>
    </div>
  );
}

/* ═══════════════════════════════════════════════════
   Root
   ═══════════════════════════════════════════════════ */

export function OverviewDashboard({
  status, compressed, docker, httpErrors, postmortem, commits, logLines,
  daemonOnline, onNavigateTriage, onNavigateRaw,
}: Props) {
  const [dismissedFocus, setDismissedFocus] = useState(false);

  if (!daemonOnline) {
    return (
      <div className="overview-root" style={{ alignItems: 'center', justifyContent: 'center' }}>
        <div className="overview-health-banner offline" style={{ maxWidth: 400 }}>
          <div className="health-indicator-dot offline" />
          <div>
            <div className="health-headline offline">Daemon Offline</div>
            <div className="health-subtext">Start the BlackBox daemon to begin monitoring.</div>
          </div>
        </div>
      </div>
    );
  }

  const traces = compressed?.stack_traces ?? [];
  const clusters = compressed?.clusters.filter(c => c.level === 'error' || c.level === 'fatal') ?? [];
  const traceCount = traces.length;
  const clusterCount = clusters.length;
  const containerCount = docker?.docker_available ? (docker?.containers.length ?? 0) : 0;
  const httpErrCount = httpErrors?.events.length ?? 0;

  const hasActiveErrors = traceCount > 0 || clusterCount > 0;
  const showFocus = hasActiveErrors && !dismissedFocus && traces.length > 0;

  // Docker errors per container
  const containerErrors: Record<string, number> = {};
  docker?.events?.forEach(e => {
    const lvl = e.level?.toLowerCase();
    if (lvl === 'error' || lvl === 'fatal' || lvl === 'critical') {
      containerErrors[e.source.container_id] = (containerErrors[e.source.container_id] ?? 0) + 1;
    }
  });

  return (
    <div className="overview-root custom-scrollbar">
      {/* 1. Inline metrics */}
      <InlineMetrics
        traces={traceCount}
        clusters={clusterCount}
        containers={containerCount}
        httpErr={httpErrCount}
        branch={status?.git_branch ?? null}
        dirty={status?.git_dirty_files ?? 0}
        uptimeSecs={status?.uptime_secs ?? 0}
      />

      {/* 2. Focus Block */}
      {showFocus && (
        <FocusBlock
          trace={traces[0]}
          moreCount={traceCount - 1 + clusterCount}
          onInspect={() => onNavigateTriage('terminal')}
          onDismiss={() => setDismissedFocus(true)}
        />
      )}

      {/* 3. Service list + Recent Commits side-by-side */}
      <div className="overview-mid-grid">
        <div>
          <div className="overview-section-label">Status</div>
          <div className="service-list">
            <ServiceRow
              dot={traceCount > 0 || clusterCount > 0 ? 'error' : 'ok'}
              name="vscode_bridge"
              type="terminal"
              status={
                traceCount > 0
                  ? `${traceCount} trace${traceCount !== 1 ? 's' : ''} · ${clusterCount} cluster${clusterCount !== 1 ? 's' : ''}`
                  : clusterCount > 0
                  ? `${clusterCount} cluster${clusterCount !== 1 ? 's' : ''}`
                  : `${status?.buffer_lines ?? 0} lines · no errors`
              }
              statusSeverity={traceCount > 0 ? 'error' : clusterCount > 0 ? 'warn' : null}
              onClick={() => onNavigateTriage('terminal')}
            />

            {docker?.docker_available && docker.containers.map(cid => {
              const errs = containerErrors[cid] ?? 0;
              const short = cid.length > 14 ? cid.slice(0, 14) : cid;
              return (
                <ServiceRow
                  key={cid}
                  dot={errs > 0 ? 'error' : 'ok'}
                  name={short}
                  type="docker"
                  status={errs > 0 ? `${errs} error${errs !== 1 ? 's' : ''}` : 'healthy'}
                  statusSeverity={errs > 0 ? 'error' : null}
                  onClick={() => onNavigateTriage('docker')}
                />
              );
            })}

            <ServiceRow
              dot={httpErrCount > 0 ? 'error' : 'ok'}
              name={`proxy :${httpErrors?.proxy_port ?? 8769}`}
              type="network"
              status={httpErrCount > 0 ? `${httpErrCount} error${httpErrCount !== 1 ? 's' : ''}` : 'no errors'}
              statusSeverity={httpErrCount > 0 ? 'error' : null}
              onClick={() => onNavigateTriage('http')}
            />
          </div>
        </div>

        <div>
          <div className="overview-section-label">
            <GitCommit size={11} /> Recent Commits
          </div>
          {commits && commits.commits.length > 0 ? (
            <div className="commit-strip">
              {commits.commits.slice(0, 5).map((c, i) => (
                <div
                  className="commit-row"
                  key={c.hash}
                  style={{
                    opacity: Math.max(0.35, 1 - i * 0.13),
                    transform: `scale(${Math.max(0.94, 1 - i * 0.012)})`,
                    transformOrigin: 'left center',
                  }}
                >
                  <span className="commit-hash">{c.hash.slice(0, 7)}</span>
                  <span className="commit-msg">{c.message}</span>
                  <span className="commit-meta">{timeAgo(c.timestamp_iso)}</span>
                </div>
              ))}
            </div>
          ) : (
            <div style={{ fontSize: '12px', color: 'var(--fg-muted)', fontFamily: "'Geist Mono', monospace", padding: '4px 0' }}>—</div>
          )}
        </div>
      </div>

      {/* 4. Full-width live logs */}
      <div>
        {postmortem && postmortem.timeline.length > 0 && (
          <MiniHistogram postmortem={postmortem} />
        )}
        <InlineLogViewer logLines={logLines} onFullscreen={onNavigateRaw} />
      </div>
    </div>
  );
}
