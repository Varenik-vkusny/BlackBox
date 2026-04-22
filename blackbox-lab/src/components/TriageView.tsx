import { useState } from 'react';
import { ArrowLeft, ChevronDown, ChevronUp, AlertOctagon, AlertTriangle, Terminal, GitBranch, Globe } from 'lucide-react';
import type { CompressedResponse, DockerResponse, HttpErrorsResponse, DiffResponse, StackTrace, LogCluster } from '../types';
import { GitLens } from './GitLens';

interface Props {
  service: string;
  compressed: CompressedResponse | null;
  docker: DockerResponse | null;
  httpErrors: HttpErrorsResponse | null;
  diff: DiffResponse | null;
  selectedSource: string | null;
  onBack: () => void;
  onNavigateRaw: () => void;
  onInspectDiff: () => void;
}

function timeAgo(ms: number): string {
  const diffSecs = Math.floor((Date.now() - ms) / 1000);
  if (diffSecs < 60) return `${diffSecs}s ago`;
  if (diffSecs < 3600) return `${Math.floor(diffSecs / 60)}m ago`;
  return `${Math.floor(diffSecs / 3600)}h ago`;
}

const LANG_SEVERITY: Record<string, number> = {
  rust: 0, go: 1, python: 2, nodejs: 3, java: 4,
};

// TODO: Change this sort to 'frequency' or 'recency' if you prefer a different triage order.
// Current: severity-first (rust panics > python > node > java), then by recency.
function sortTraces(traces: StackTrace[]): StackTrace[] {
  return [...traces].sort((a, b) => {
    const severityA = LANG_SEVERITY[a.language] ?? 99;
    const severityB = LANG_SEVERITY[b.language] ?? 99;
    if (severityA !== severityB) return severityA - severityB;
    return b.captured_at_ms - a.captured_at_ms;
  });
}

function langColor(lang: string): string {
  switch (lang) {
    case 'rust':   return 'var(--accent-orange)';
    case 'python': return 'var(--accent-cyan)';
    case 'nodejs': return 'var(--accent-green)';
    case 'java':   return 'var(--accent-yellow)';
    case 'go':     return 'var(--accent-cyan)';
    default:       return 'var(--text-secondary)';
  }
}

function langBg(lang: string): string {
  switch (lang) {
    case 'rust':   return 'rgba(249,115,22,0.15)';
    case 'python': return 'rgba(34,211,238,0.12)';
    case 'nodejs': return 'rgba(34,197,94,0.12)';
    case 'java':   return 'rgba(234,179,8,0.12)';
    case 'go':     return 'rgba(6,182,212,0.12)';
    default:       return 'rgba(148,163,184,0.1)';
  }
}

function isCritical(trace: StackTrace): boolean {
  return trace.language === 'rust' || trace.language === 'go';
}

interface TraceCardProps {
  trace: StackTrace;
  onInspectDiff: () => void;
}

function TraceCard({ trace, onInspectDiff }: TraceCardProps) {
  const [expanded, setExpanded] = useState(false);
  const userFrames = trace.frames.filter(f => f.is_user_code);
  const critical = isCritical(trace);

  return (
    <div className={`error-card ${expanded ? 'expanded' : ''}`}>
      <div className="error-card-header" onClick={() => setExpanded(e => !e)}>
        <span className="error-card-icon" style={{ color: critical ? 'var(--accent-red)' : 'var(--accent-orange)' }}>
          {critical ? <AlertOctagon size={14} /> : <AlertTriangle size={14} />}
        </span>
        <div className="error-card-title">
          <div className="error-card-title-text">{trace.error_message}</div>
          <div className="error-card-title-meta">
            <span style={{
              padding: '0.1rem 0.4rem',
              borderRadius: '9999px',
              fontSize: '0.55rem',
              fontWeight: 700,
              background: langBg(trace.language),
              color: langColor(trace.language),
              textTransform: 'uppercase',
              letterSpacing: '0.08em',
            }}>
              {trace.language}
            </span>
            <span>{userFrames.length} user frame{userFrames.length !== 1 ? 's' : ''}</span>
            <span>·</span>
            <span>{timeAgo(trace.captured_at_ms)}</span>
          </div>
        </div>
        <span className={`error-card-count ${critical ? '' : 'warn'}`}>
          {critical ? 'PANIC' : 'ERROR'}
        </span>
        <span className="error-card-chevron">
          {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
        </span>
      </div>

      {expanded && (
        <div className="error-card-body">
          {userFrames.length > 0 && (
            <div className="error-card-frames">
              {trace.frames.map((frame, i) => (
                <div key={i} className={`error-card-frame ${frame.is_user_code ? 'user' : ''}`}>
                  <span style={{ color: 'var(--text-muted)', marginRight: '0.5rem' }}>{i + 1}.</span>
                  {frame.file && frame.line
                    ? <span>{frame.file}<span style={{ color: 'var(--accent-cyan)' }}>:{frame.line}</span></span>
                    : frame.raw}
                </div>
              ))}
            </div>
          )}
          {trace.source_files.length > 0 && (
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.35rem', marginBottom: '0.5rem' }}>
              {trace.source_files.map(f => (
                <span key={f} style={{
                  fontSize: '0.6rem', fontFamily: 'JetBrains Mono, monospace',
                  color: 'var(--accent-cyan)', background: 'rgba(34,211,238,0.08)',
                  padding: '0.15rem 0.45rem', borderRadius: '9999px', border: '1px solid rgba(34,211,238,0.15)',
                }}>
                  {f.split('/').pop()}
                </span>
              ))}
            </div>
          )}
          <div className="error-card-actions">
            <button className="inspect-btn" onClick={e => { e.stopPropagation(); onInspectDiff(); }}>
              <GitBranch size={11} /> Inspect Changes
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

interface ClusterCardProps {
  cluster: LogCluster;
}

function ClusterCard({ cluster }: ClusterCardProps) {
  const [expanded, setExpanded] = useState(false);
  return (
    <div className="triage-cluster-card" style={{ cursor: 'pointer', flexDirection: 'column', alignItems: 'stretch' }}
      onClick={() => setExpanded(e => !e)}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '0.625rem' }}>
        <span className="triage-cluster-count">×{cluster.count}</span>
        <span className="triage-cluster-pattern">{cluster.pattern}</span>
        <span className="triage-cluster-ago">{timeAgo(cluster.last_seen_ms)}</span>
      </div>
      {expanded && (
        <div style={{
          marginTop: '0.5rem', padding: '0.5rem',
          fontFamily: 'JetBrains Mono, monospace', fontSize: '0.65rem',
          color: 'var(--text-secondary)', background: 'rgba(0,0,0,0.2)',
          borderRadius: 'var(--radius-sm)', borderTop: '1px solid var(--border)',
        }}>
          <div style={{ color: 'var(--text-muted)', marginBottom: '0.25rem', fontSize: '0.58rem', textTransform: 'uppercase', letterSpacing: '0.08em' }}>Example</div>
          {cluster.example}
        </div>
      )}
    </div>
  );
}

function serviceLabel(service: string, selectedSource: string | null): string {
  if (service === 'docker' && selectedSource?.startsWith('docker:')) {
    return `Container: ${selectedSource.slice(7).slice(0, 12)}`;
  }
  switch (service) {
    case 'terminal': return 'Terminal / vscode_bridge';
    case 'docker':   return 'Docker Containers';
    case 'http':     return 'HTTP Proxy';
    default:         return service;
  }
}

function serviceIcon(service: string): React.ReactNode {
  switch (service) {
    case 'terminal': return <Terminal size={14} />;
    case 'docker':   return <Globe size={14} />;
    default:         return <Globe size={14} />;
  }
}

export function TriageView({
  service,
  compressed,
  docker,
  httpErrors,
  diff,
  selectedSource,
  onBack,
  onNavigateRaw,
  onInspectDiff
}: Props) {
  const [showDiff, setShowDiff] = useState(false);

  const handleInspectDiff = () => {
    onInspectDiff();
    setShowDiff(true);
  };

  const sortedTraces = sortTraces(
    service === 'http'
      ? []
      : (compressed?.stack_traces ?? [])
  );

  const clusters: LogCluster[] = (() => {
    let cl = (compressed?.clusters ?? []).filter(c => c.level === 'error' || c.level === 'fatal' || c.level === 'warn' || c.count > 3);
    
    if (service === 'terminal') {
      cl = cl.filter(c => !c.sources || c.sources.length === 0 || !c.sources.some(s => s.startsWith('docker:')));
    } else if (service === 'docker') {
      if (selectedSource?.startsWith('docker:')) {
        const cid = selectedSource.slice(7);
        cl = cl.filter(c => c.sources && c.sources.some(s => s.startsWith(`docker:${cid}`) || s.startsWith(`docker:${cid.slice(0, 12)}`)));
      } else {
        cl = cl.filter(c => c.sources && c.sources.some(s => s.startsWith('docker:')));
      }
    } else {
      cl = [];
    }
    return cl;
  })();

  const dockerErrorEvents = service === 'docker'
    ? (docker?.events.filter(e => {
        const lvl = e.level?.toLowerCase();
        const isErr = lvl === 'error' || lvl === 'fatal' || lvl === 'critical';
        if (!isErr) return false;
        
        if (selectedSource?.startsWith('docker:')) {
          const cid = selectedSource.slice(7);
          return e.source.container_id.startsWith(cid) || cid.startsWith(e.source.container_id.slice(0, 12));
        }
        return true;
      }) ?? [])
    : [];

  const httpErrorEvents = service === 'http'
    ? (httpErrors?.events ?? [])
    : [];

  const totalItems = sortedTraces.length + clusters.length + dockerErrorEvents.length + httpErrorEvents.length;
  const hasContent = totalItems > 0;

  return (
    <div className="triage-root">
      {/* Header */}
      <div className="triage-header">
        <button className="triage-back-btn" onClick={onBack}>
          <ArrowLeft size={12} /> Overview
        </button>
        <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', flex: 1 }}>
          <span style={{ color: 'var(--text-muted)' }}>{serviceIcon(service)}</span>
          <span className="triage-service-badge">{serviceLabel(service, selectedSource)}</span>
        </div>
        <span className="triage-stats">
          {sortedTraces.length > 0 && `${sortedTraces.length} trace${sortedTraces.length !== 1 ? 's' : ''}`}
          {clusters.length > 0 && ` · ${clusters.length} cluster${clusters.length !== 1 ? 's' : ''}`}
        </span>
        <button className="btn btn-ghost" onClick={onNavigateRaw}
          style={{ minHeight: 28, padding: '0.2rem 0.6rem', fontSize: '0.62rem' }}>
          <Terminal size={11} /> Raw Logs
        </button>
      </div>

      {/* Body */}
      <div className="triage-body custom-scrollbar">
        {!hasContent && (
          <div className="triage-empty">
            <div className="triage-empty-icon" style={{ fontSize: '2rem', opacity: 0.3 }}>✓</div>
            <div style={{ fontWeight: 700, color: 'var(--accent-green)' }}>No errors detected</div>
            <div style={{ color: 'var(--text-muted)', fontSize: '0.68rem' }}>
              {service === 'docker' && !docker?.docker_available
                ? 'Docker is not reachable'
                : 'This service is running cleanly'}
            </div>
            <button className="btn btn-ghost" onClick={onNavigateRaw}
              style={{ marginTop: '0.5rem', border: '1px solid var(--border)' }}>
              <Terminal size={12} /> Open Raw Logs
            </button>
          </div>
        )}

        {/* Stack traces (terminal service) */}
        {sortedTraces.length > 0 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.625rem' }}>
            <div className="overview-section-label" style={{ marginBottom: 0 }}>
              <AlertOctagon size={11} /> Stack Traces ({sortedTraces.length})
            </div>
            {sortedTraces.map((trace, i) => (
              <TraceCard key={i} trace={trace} onInspectDiff={handleInspectDiff} />
            ))}
          </div>
        )}

        {/* Log clusters */}
        {clusters.length > 0 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.375rem', marginTop: sortedTraces.length > 0 ? '0.5rem' : 0 }}>
            <div className="overview-section-label" style={{ marginBottom: 0 }}>
              <AlertTriangle size={11} /> Repeated Errors ({clusters.length})
            </div>
            {clusters.map((c, i) => <ClusterCard key={i} cluster={c} />)}
          </div>
        )}

        {/* Docker error events */}
        {dockerErrorEvents.length > 0 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.375rem' }}>
            <div className="overview-section-label" style={{ marginBottom: 0 }}>
              <AlertOctagon size={11} /> Container Errors ({dockerErrorEvents.length})
            </div>
            {dockerErrorEvents.map((ev, i) => (
              <div key={i} className="triage-cluster-card" style={{ flexDirection: 'column', alignItems: 'flex-start', gap: '0.25rem', borderLeftColor: 'rgba(244,63,94,0.4)' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', width: '100%' }}>
                  <span className="triage-cluster-count" style={{ background: 'rgba(244,63,94,0.12)', color: 'var(--accent-red)' }}>
                    {ev.level}
                  </span>
                  <span style={{ fontSize: '0.6rem', color: 'var(--accent-orange)', fontFamily: 'JetBrains Mono, monospace' }}>
                    {ev.source.container_id.slice(0, 12)}
                  </span>
                  <span className="triage-cluster-ago" style={{ marginLeft: 'auto' }}>
                    {timeAgo(ev.timestamp_ms)}
                  </span>
                </div>
                <span className="triage-cluster-pattern" style={{ whiteSpace: 'normal', fontSize: '0.68rem' }}>{ev.text}</span>
              </div>
            ))}
          </div>
        )}

        {/* HTTP errors */}
        {httpErrorEvents.length > 0 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '0.375rem' }}>
            <div className="overview-section-label" style={{ marginBottom: 0 }}>
              <Globe size={11} /> HTTP Errors ({httpErrorEvents.length})
            </div>
            {httpErrorEvents.map((ev, i) => (
              <div key={i} className="triage-cluster-card" style={{ flexDirection: 'column', alignItems: 'flex-start', gap: '0.25rem', borderLeftColor: ev.status >= 500 ? 'rgba(244,63,94,0.4)' : 'rgba(249,115,22,0.4)' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', width: '100%' }}>
                  <span style={{
                    fontSize: '0.62rem', fontWeight: 700, fontFamily: 'JetBrains Mono, monospace',
                    padding: '0.1rem 0.4rem', borderRadius: 3,
                    background: 'rgba(148,163,184,0.1)', color: 'var(--text-secondary)',
                  }}>
                    {ev.method}
                  </span>
                  <span style={{ fontFamily: 'JetBrains Mono, monospace', fontSize: '0.7rem', fontWeight: 700, color: ev.status >= 500 ? 'var(--accent-red)' : 'var(--accent-orange)' }}>
                    {ev.status}
                  </span>
                  <span className="triage-cluster-pattern">{ev.url}</span>
                  <span className="triage-cluster-ago">{ev.latency_ms}ms</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>

      {showDiff && diff && (
        <div className="diff-overlay">
          <div className="card-header">
            <span className="card-title">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" /><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" /></svg>
              Surgical Diff
            </span>
            <div className="spacer" />
            <button className="btn btn-ghost" style={{ padding: '0.2rem 0.5rem', minHeight: 28 }} onClick={() => setShowDiff(false)}>✕</button>
          </div>
          <div style={{ flex: 1, overflow: 'hidden' }}>
            <GitLens diff={diff} onRefresh={onInspectDiff} loading={false} />
          </div>
        </div>
      )}
    </div>
  );
}
