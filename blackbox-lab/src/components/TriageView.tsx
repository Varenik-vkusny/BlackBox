import { useState } from 'react';
import { ArrowLeft, ChevronDown, ChevronUp, GitBranch, FileText } from 'lucide-react';
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

function isCritical(trace: StackTrace): boolean {
  return trace.language === 'rust' || trace.language === 'go';
}

/* ═══════════════════════════════════════════════════
   Trace Card
   ═══════════════════════════════════════════════════ */

interface TraceCardProps {
  trace: StackTrace;
  defaultExpanded: boolean;
  onInspectDiff: () => void;
}

function TraceCard({ trace, defaultExpanded, onInspectDiff }: TraceCardProps) {
  const [expanded, setExpanded] = useState(defaultExpanded);
  const userFrames = trace.frames.filter(f => f.is_user_code);
  const critical = isCritical(trace);

  return (
    <div className={`error-card ${expanded ? 'expanded' : ''}`}>
      <div className="error-card-header" onClick={() => setExpanded(e => !e)}>
        <div className="error-card-title">
          <div className="error-card-title-text">{trace.error_message}</div>
          <div className="error-card-title-meta">
            <span style={{
              padding: '2px 8px', borderRadius: '4px', fontSize: '11px', fontWeight: 500,
              background: 'var(--bg-raised)', color: 'var(--fg-secondary)',
              textTransform: 'uppercase', letterSpacing: '0.04em', fontFamily: "'Geist Mono', monospace",
            }}>
              {trace.language}
            </span>
            <span>{userFrames.length} user frame{userFrames.length !== 1 ? 's' : ''}</span>
            <span>·</span>
            <span>{timeAgo(trace.captured_at_ms)}</span>
          </div>
        </div>
        <div className="error-card-right">
          <span className="error-card-count">
            {critical ? 'PANIC' : 'ERROR'}
          </span>
          <button
            className="btn btn-ghost"
            onClick={e => { e.stopPropagation(); onInspectDiff(); }}
            style={{ minHeight: 24, padding: '2px 8px', fontSize: '11px' }}
          >
            <GitBranch size={10} /> Inspect Changes
          </button>
          <span className="error-card-chevron">
            {expanded ? <ChevronUp size={14} /> : <ChevronDown size={14} />}
          </span>
        </div>
      </div>

      {expanded && (
        <div className="error-card-body">
          {userFrames.length > 0 && (
            <div className="error-card-frames">
              {trace.frames.map((frame, i) => (
                <div key={i} className={`error-card-frame ${frame.is_user_code ? 'user' : ''}`}>
                  <span style={{ color: 'var(--fg-muted)', marginRight: '0.5rem' }}>{i + 1}.</span>
                  {frame.file && frame.line
                    ? <span>{frame.file}<span style={{ color: 'var(--fg-secondary)' }}>:{frame.line}</span></span>
                    : frame.raw}
                </div>
              ))}
            </div>
          )}
          {trace.source_files.length > 0 && (
            <div style={{ display: 'flex', flexWrap: 'wrap', gap: '4px', marginBottom: '8px' }}>
              {trace.source_files.map(f => (
                <span
                  key={f}
                  title="Open in editor"
                  style={{
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: '4px',
                    fontSize: '11px',
                    fontFamily: "'Geist Mono', monospace",
                    color: 'var(--fg-secondary)',
                    background: 'var(--bg-raised)',
                    padding: '2px 8px',
                    borderRadius: '4px',
                    border: '1px solid var(--border-subtle)',
                    cursor: 'default',
                  }}
                >
                  <FileText size={10} />
                  {f.split('/').slice(-2).join('/')}
                </span>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

/* ═══════════════════════════════════════════════════
   Cluster Card
   ═══════════════════════════════════════════════════ */

function HighlightPattern({ pattern }: { pattern: string }) {
  // Render * tokens with muted background so they read as placeholders
  const tokens = pattern.split(/(\*+)/g);
  return (
    <span>
      {tokens.map((t, i) =>
        t.startsWith('*') ? (
          <span
            key={i}
            style={{
              background: 'var(--bg-raised)',
              color: 'var(--fg-disabled)',
              padding: '0 3px',
              borderRadius: '3px',
              fontSize: '0.9em',
              lineHeight: 1,
            }}
          >
            {t}
          </span>
        ) : (
          <span key={i}>{t}</span>
        )
      )}
    </span>
  );
}

interface ClusterCardProps {
  cluster: LogCluster;
}

function ClusterCard({ cluster }: ClusterCardProps) {
  const [expanded, setExpanded] = useState(false);
  const example = cluster.example?.slice(0, 80) ?? '';

  return (
    <div className="triage-cluster-card" style={{ cursor: 'pointer', flexDirection: 'column', alignItems: 'stretch' }}
      onClick={() => setExpanded(e => !e)}>
      <div style={{ display: 'flex', alignItems: 'center', gap: '10px' }}>
        <span style={{
          fontSize: '11px', fontWeight: 500, fontFamily: "'Geist Mono', monospace",
          color: 'var(--fg-secondary)', background: 'var(--bg-raised)',
          padding: '2px 8px', borderRadius: '4px', flexShrink: 0,
        }}>
          ×{cluster.count}
        </span>
        <span className="triage-cluster-pattern" style={{ fontFamily: "'Geist Mono', monospace" }}>
          <HighlightPattern pattern={cluster.pattern} />
        </span>
        <span className="triage-cluster-ago">{timeAgo(cluster.last_seen_ms)}</span>
      </div>
      {!expanded && example && (
        <div style={{
          marginTop: '4px', paddingLeft: '42px',
          fontFamily: "'Geist Mono', monospace", fontSize: '11px',
          color: 'var(--fg-muted)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis',
        }}>
          e.g. {example}
        </div>
      )}
      {expanded && (
        <div style={{
          marginTop: '8px', padding: '8px',
          fontFamily: "'Geist Mono', monospace", fontSize: '12px',
          color: 'var(--fg-secondary)', background: 'var(--bg-raised)',
          borderRadius: 'var(--radius-sm)', border: '1px solid var(--border-subtle)',
        }}>
          <div style={{ color: 'var(--fg-muted)', fontSize: '11px', marginBottom: '4px', textTransform: 'uppercase', letterSpacing: '0.08em' }}>Example</div>
          {cluster.example}
        </div>
      )}
    </div>
  );
}

/* ═══════════════════════════════════════════════════
   Root
   ═══════════════════════════════════════════════════ */

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

  const sortedTraces = (() => {
    const traces = service === 'http' ? [] : (compressed?.stack_traces ?? []);
    const LANG_SEVERITY: Record<string, number> = { rust: 0, go: 1, python: 2, nodejs: 3, java: 4 };
    return [...traces].sort((a, b) => {
      const sa = LANG_SEVERITY[a.language] ?? 99;
      const sb = LANG_SEVERITY[b.language] ?? 99;
      if (sa !== sb) return sa - sb;
      return b.captured_at_ms - a.captured_at_ms;
    });
  })();

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
        <span className="triage-service-badge">
          {service === 'terminal' ? 'vscode_bridge' : service === 'docker' ? 'docker' : 'http-proxy'}
        </span>
        <span className="triage-stats">
          {sortedTraces.length > 0 && `${sortedTraces.length} trace${sortedTraces.length !== 1 ? 's' : ''}`}
          {clusters.length > 0 && ` · ${clusters.length} cluster${clusters.length !== 1 ? 's' : ''}`}
        </span>
        <button className="btn btn-ghost" onClick={onNavigateRaw}
          style={{ minHeight: 28, padding: '0.2rem 0.6rem', fontSize: '0.62rem' }}
        >
          Raw Logs
        </button>
      </div>

      {/* Body */}
      <div className="triage-body custom-scrollbar">
        {!hasContent && (
          <div className="triage-empty">
            <div style={{ color: 'var(--fg-muted)', fontSize: '12px' }}>No errors detected.</div>
          </div>
        )}

        {sortedTraces.length > 0 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
            <div className="overview-section-label" style={{ marginBottom: 0 }}>
              Stack Traces ({sortedTraces.length})
            </div>
            {sortedTraces.map((trace, i) => (
              <TraceCard key={i} trace={trace} defaultExpanded={i === 0} onInspectDiff={handleInspectDiff} />
            ))}
          </div>
        )}

        {clusters.length > 0 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '6px', marginTop: sortedTraces.length > 0 ? '8px' : 0 }}>
            <div className="overview-section-label" style={{ marginBottom: 0 }}>
              Repeated Errors ({clusters.length})
            </div>
            {clusters.map((c, i) => <ClusterCard key={i} cluster={c} />)}
          </div>
        )}

        {dockerErrorEvents.length > 0 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
            <div className="overview-section-label" style={{ marginBottom: 0 }}>
              Container Errors ({dockerErrorEvents.length})
            </div>
            {dockerErrorEvents.map((ev, i) => (
              <div key={i} className="triage-cluster-card" style={{ flexDirection: 'column', alignItems: 'flex-start', gap: '4px', borderLeftColor: 'rgba(229,72,77,0.4)' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px', width: '100%' }}>
                  <span style={{ fontSize: '11px', fontWeight: 500, fontFamily: "'Geist Mono', monospace", background: 'rgba(229,72,77,0.12)', color: 'var(--severity-error)', padding: '2px 8px', borderRadius: '4px' }}>
                    {ev.level}
                  </span>
                  <span style={{ fontSize: '12px', color: 'var(--fg-secondary)', fontFamily: "'Geist Mono', monospace" }}>
                    {ev.source.container_id.slice(0, 12)}
                  </span>
                  <span className="triage-cluster-ago" style={{ marginLeft: 'auto' }}>{timeAgo(ev.timestamp_ms)}</span>
                </div>
                <span className="triage-cluster-pattern" style={{ whiteSpace: 'normal', fontSize: '12px' }}>{ev.text}</span>
              </div>
            ))}
          </div>
        )}

        {httpErrorEvents.length > 0 && (
          <div style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}>
            <div className="overview-section-label" style={{ marginBottom: 0 }}>
              HTTP Errors ({httpErrorEvents.length})
            </div>
            {httpErrorEvents.map((ev, i) => (
              <div key={i} className="triage-cluster-card" style={{ flexDirection: 'column', alignItems: 'flex-start', gap: '4px', borderLeftColor: ev.status >= 500 ? 'rgba(229,72,77,0.4)' : 'rgba(217,119,87,0.4)' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '8px', width: '100%' }}>
                  <span style={{
                    fontSize: '11px', fontWeight: 500, fontFamily: "'Geist Mono', monospace",
                    padding: '2px 8px', borderRadius: '4px',
                    background: 'var(--bg-raised)', color: 'var(--fg-secondary)',
                  }}>
                    {ev.method}
                  </span>
                  <span style={{ fontFamily: "'Geist Mono', monospace", fontSize: '13px', fontWeight: 500, color: ev.status >= 500 ? 'var(--severity-error)' : 'var(--severity-warn)' }}>
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
              Contextual Diff
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
