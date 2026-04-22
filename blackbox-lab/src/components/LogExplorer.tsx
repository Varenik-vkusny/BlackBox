import React, { useState, useRef, useEffect } from 'react';
import { Terminal, Filter, Layers, Box, AlertCircle, FileCode, Clock, GitMerge } from 'lucide-react';
import type { CompressedResponse, DockerResponse, PostmortemResponse, CorrelatedResponse } from '../types';

interface LogExplorerProps {
  raw: string[];
  compressed: CompressedResponse | null;
  docker: DockerResponse | null;
  postmortem: PostmortemResponse | null;
  correlated: CorrelatedResponse | null;
}

type Mode = 'raw' | 'compressed' | 'docker' | 'postmortem';

function logLineClass(text: string): string {
  const t = text.toLowerCase();
  if (/\b(error|panic|fatal|exception|fail)\b/.test(t)) return 'log-error';
  if (/\b(warn|warning|deprecated)\b/.test(t)) return 'log-warn';
  if (/\b(info|note|success)\b/.test(t)) return 'log-info';
  if (/\b(debug|trace)\b/.test(t)) return 'log-debug';
  return 'log-normal';
}

function levelBadgeClass(level: string | null): string {
  const l = (level || '').toLowerCase();
  if (l === 'error' || l === 'fatal') return 'badge badge-red';
  if (l === 'warn' || l === 'warning') return 'badge badge-orange';
  if (l === 'info') return 'badge badge-cyan';
  return 'badge badge-muted';
}

export const LogExplorer: React.FC<LogExplorerProps> = ({
  raw,
  compressed,
  docker,
  postmortem,
  correlated,
}) => {
  const [mode, setMode] = useState<Mode>('raw');
  const [filter, setFilter] = useState('');
  const bottomRef = useRef<HTMLDivElement>(null);
  const bodyRef = useRef<HTMLDivElement>(null);
  const [autoScroll, setAutoScroll] = useState(true);

  useEffect(() => {
    if (mode === 'raw' && autoScroll && bottomRef.current) {
      bottomRef.current.scrollIntoView({ behavior: 'smooth' });
    }
  }, [raw, mode, autoScroll]);

  const handleScroll = () => {
    if (!bodyRef.current) return;
    const { scrollTop, scrollHeight, clientHeight } = bodyRef.current;
    setAutoScroll(scrollHeight - scrollTop - clientHeight < 60);
  };

  const filteredRaw = raw.filter((l) => l.toLowerCase().includes(filter.toLowerCase()));

  const renderRaw = () => (
    <div style={{ padding: '0.5rem 0' }}>
      {filteredRaw.length === 0 && (
        <Empty icon={<Terminal size={28} />} message="No log lines yet — inject a payload above" />
      )}
      {filteredRaw.map((log, i) => (
        <div
          key={i}
          style={{
            display: 'flex',
            gap: '0.75rem',
            padding: '0.2rem 1.25rem',
            transition: 'background 0.1s',
          }}
          onMouseEnter={(e) => (e.currentTarget.style.background = 'rgba(255,255,255,0.03)')}
          onMouseLeave={(e) => (e.currentTarget.style.background = 'transparent')}
        >
          <span
            className="mono"
            style={{
              fontSize: '0.62rem',
              color: 'var(--text-muted)',
              width: 32,
              textAlign: 'right',
              flexShrink: 0,
              paddingTop: 2,
              userSelect: 'none',
            }}
          >
            {filteredRaw.length - i}
          </span>
          <span
            className={`mono ${logLineClass(log)}`}
            style={{ fontSize: '0.72rem', lineHeight: 1.6, wordBreak: 'break-all' }}
          >
            {log}
          </span>
        </div>
      ))}
      <div ref={bottomRef} />
    </div>
  );

  const renderCompressed = () => {
    if (!compressed) return <Loading />;
    const hasTraces = compressed.stack_traces.length > 0;
    const hasClusters = compressed.clusters.length > 0;
    if (!hasTraces && !hasClusters) {
      return <Empty icon={<Layers size={28} />} message="No clusters or stack traces — inject some errors first" />;
    }
    return (
      <div className="p-body" style={{ display: 'flex', flexDirection: 'column', gap: '1.25rem' }}>
        {hasTraces && (
          <section>
            <SectionLabel icon={<FileCode size={11} />} text="Detected Stack Traces" color="var(--accent-red)" />
            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.625rem', marginTop: '0.5rem' }}>
              {compressed.stack_traces.map((st, i) => (
                <div
                  key={i}
                  style={{
                    borderRadius: '0.5rem',
                    border: '1px solid rgba(244,63,94,0.15)',
                    background: 'rgba(244,63,94,0.04)',
                    overflow: 'hidden',
                  }}
                >
                  <div
                    style={{
                      padding: '0.5rem 0.875rem',
                      borderBottom: '1px solid rgba(244,63,94,0.1)',
                      display: 'flex',
                      alignItems: 'center',
                      gap: '0.5rem',
                    }}
                  >
                    <AlertCircle size={12} style={{ color: 'var(--accent-red)', flexShrink: 0 }} />
                    <span
                      className="mono truncate"
                      style={{ fontSize: '0.72rem', fontWeight: 600, color: 'var(--accent-red)', flex: 1 }}
                    >
                      {st.error_message}
                    </span>
                    <span className="badge badge-red">{st.language}</span>
                  </div>
                  <div style={{ padding: '0.5rem 0.875rem' }}>
                    {st.frames.map((f, fi) => (
                      <div
                        key={fi}
                        className="mono"
                        style={{
                          display: 'flex',
                          gap: '0.75rem',
                          fontSize: '0.65rem',
                          lineHeight: 1.7,
                          color: f.is_user_code ? 'var(--text-primary)' : 'var(--text-muted)',
                          opacity: f.is_user_code ? 1 : 0.5,
                        }}
                      >
                        <span style={{ width: 48, textAlign: 'right', flexShrink: 0, color: 'var(--text-muted)' }}>
                          {f.line ?? '–'}
                        </span>
                        <span className="truncate" style={{ width: 160, flexShrink: 0 }}>
                          {f.file || 'internal'}
                        </span>
                        <span className="truncate">{f.raw}</span>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          </section>
        )}

        {hasClusters && (
          <section>
            <SectionLabel icon={<Layers size={11} />} text={`Log Clusters — Drain (${compressed.clusters.length})`} color="var(--text-secondary)" />
            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.375rem', marginTop: '0.5rem' }}>
              {compressed.clusters.map((c, i) => (
                <div
                  key={i}
                  style={{
                    display: 'flex',
                    gap: '0.875rem',
                    alignItems: 'center',
                    padding: '0.625rem 0.875rem',
                    borderRadius: '0.4rem',
                    border: '1px solid var(--border)',
                    background: 'rgba(0,0,0,0.2)',
                  }}
                >
                  <div
                    style={{
                      minWidth: 40,
                      textAlign: 'center',
                      fontWeight: 700,
                      fontSize: '0.9rem',
                      color: 'var(--text-primary)',
                      fontFamily: "'JetBrains Mono', monospace",
                    }}
                  >
                    {c.count}
                  </div>
                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{ display: 'flex', alignItems: 'center', gap: '0.375rem', marginBottom: '0.2rem' }}>
                      <span className={levelBadgeClass(c.level)}>{c.level || 'info'}</span>
                      <span className="mono truncate" style={{ fontSize: '0.68rem', color: 'var(--text-secondary)' }}>
                        {c.pattern}
                      </span>
                    </div>
                    <p
                      className="mono truncate"
                      style={{ fontSize: '0.65rem', color: 'var(--text-muted)', fontStyle: 'italic' }}
                    >
                      {c.example}
                    </p>
                  </div>
                </div>
              ))}
            </div>
          </section>
        )}
      </div>
    );
  };

  const renderDocker = () => {
    if (!docker) return <Loading />;
    if (!docker.docker_available)
      return <Empty icon={<Box size={28} />} message="Docker daemon not reachable — start Docker Desktop" />;
    if (docker.events.length === 0)
      return <Empty icon={<Box size={28} />} message="No container errors detected" />;
    return (
      <div className="p-body" style={{ display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
        {docker.events.map((e, i) => (
          <div
            key={i}
            style={{
              padding: '0.625rem 0.875rem',
              borderRadius: '0.4rem',
              border: '1px solid rgba(244,63,94,0.15)',
              borderLeft: '3px solid var(--accent-red)',
              background: 'rgba(244,63,94,0.04)',
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: '0.5rem', marginBottom: '0.35rem' }}>
              <span className="badge badge-red" style={{ fontFamily: "'JetBrains Mono', monospace" }}>
                {e.source.container_id.slice(0, 12)}
              </span>
              <span className="mono" style={{ fontSize: '0.62rem', color: 'var(--text-muted)', marginLeft: 'auto' }}>
                {new Date(e.timestamp_ms).toLocaleTimeString()}
              </span>
            </div>
            <p className="mono" style={{ fontSize: '0.7rem', color: 'var(--text-secondary)', wordBreak: 'break-all' }}>
              {e.text}
            </p>
          </div>
        ))}
      </div>
    );
  };

  const renderPostmortem = () => {
    if (!postmortem) return <Loading />;

    return (
      <div className="p-body" style={{ display: 'flex', flexDirection: 'column', gap: '1.25rem' }}>
        {/* Summary row */}
        <div style={{ display: 'flex', gap: '0.75rem', flexWrap: 'wrap' }}>
          {[
            { label: 'Window', value: `${postmortem.window_minutes}m` },
            { label: 'Total Lines', value: postmortem.total_lines },
            { label: 'Buckets', value: postmortem.timeline.length },
            { label: 'Docker Events', value: postmortem.docker_events_in_window },
            { label: 'Stack Traces', value: postmortem.stack_traces.length },
          ].map((s) => (
            <div
              key={s.label}
              style={{
                padding: '0.5rem 0.875rem',
                borderRadius: '0.4rem',
                border: '1px solid var(--border)',
                background: 'rgba(0,0,0,0.2)',
                textAlign: 'center',
              }}
            >
              <div style={{ fontSize: '1rem', fontWeight: 700, fontFamily: "'JetBrains Mono', monospace" }}>
                {s.value}
              </div>
              <div style={{ fontSize: '0.6rem', color: 'var(--text-muted)', textTransform: 'uppercase', letterSpacing: '0.08em' }}>
                {s.label}
              </div>
            </div>
          ))}
        </div>

        {/* Timeline */}
        {postmortem.timeline.length > 0 ? (
          <section>
            <SectionLabel icon={<Clock size={11} />} text="Activity Timeline (per minute)" color="var(--accent-cyan)" />
            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.3rem', marginTop: '0.5rem' }}>
              {postmortem.timeline.map((bucket) => {
                const errorRatio = bucket.error_count / Math.max(bucket.line_count, 1);
                const barWidth = `${Math.max((bucket.line_count / Math.max(postmortem.total_lines * 0.3, 1)) * 100, 4)}%`;
                const barColor =
                  bucket.error_count > 0 ? 'var(--accent-red)' : 'var(--accent-green)';
                return (
                  <div
                    key={bucket.minute_offset}
                    style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}
                  >
                    <span
                      className="mono"
                      style={{ fontSize: '0.62rem', color: 'var(--text-muted)', width: 28, textAlign: 'right', flexShrink: 0 }}
                    >
                      +{bucket.minute_offset}m
                    </span>
                    <div
                      style={{
                        height: 20,
                        width: barWidth,
                        minWidth: 8,
                        maxWidth: '55%',
                        borderRadius: 3,
                        background: barColor,
                        opacity: 0.6 + errorRatio * 0.4,
                        transition: 'width 0.3s ease',
                        flexShrink: 0,
                      }}
                    />
                    <span className="mono truncate" style={{ fontSize: '0.62rem', color: 'var(--text-muted)' }}>
                      {bucket.line_count} lines
                      {bucket.error_count > 0 && (
                        <span style={{ color: 'var(--accent-red)', marginLeft: '0.35rem' }}>
                          · {bucket.error_count} err
                        </span>
                      )}
                    </span>
                  </div>
                );
              })}
            </div>
          </section>
        ) : (
          <Empty icon={<Clock size={28} />} message="No timeline data — inject some logs first" />
        )}

        {/* Correlated cross-source */}
        {correlated && correlated.has_cross_source_correlations && (
          <section>
            <SectionLabel
              icon={<GitMerge size={11} />}
              text="Cross-source Correlations"
              color="var(--accent-orange)"
            />
            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.375rem', marginTop: '0.5rem' }}>
              {correlated.correlations
                .filter((c) => c.correlated_docker_events.length > 0)
                .slice(0, 10)
                .map((c, i) => (
                  <div
                    key={i}
                    style={{
                      padding: '0.625rem 0.875rem',
                      borderRadius: '0.4rem',
                      border: '1px solid rgba(249,115,22,0.15)',
                      background: 'rgba(249,115,22,0.04)',
                    }}
                  >
                    <p className="mono truncate" style={{ fontSize: '0.68rem', color: 'var(--text-secondary)', marginBottom: '0.3rem' }}>
                      {c.terminal_line}
                    </p>
                    {c.correlated_docker_events.map((de, di) => (
                      <p key={di} className="mono truncate" style={{ fontSize: '0.62rem', color: 'var(--accent-orange)', opacity: 0.8 }}>
                        ↳ docker:{de.source} — {de.text}
                      </p>
                    ))}
                  </div>
                ))}
            </div>
          </section>
        )}
      </div>
    );
  };

  const renderContent = () => {
    switch (mode) {
      case 'raw':        return renderRaw();
      case 'compressed': return renderCompressed();
      case 'docker':     return renderDocker();
      case 'postmortem': return renderPostmortem();
    }
  };

  return (
    <div className="card" style={{ height: 700 }}>
      <div className="card-header" style={{ gap: '0.75rem', flexWrap: 'wrap' }}>
        <Terminal size={14} style={{ color: 'var(--accent-cyan)', flexShrink: 0 }} />
        <span className="card-title">Explorer</span>

        <div className="tab-row" style={{ marginLeft: '0.25rem' }}>
          {(
            [
              { id: 'raw',        icon: <Terminal size={11} />,  label: 'Raw' },
              { id: 'compressed', icon: <Layers size={11} />,    label: 'Analyzed' },
              { id: 'docker',     icon: <Box size={11} />,       label: 'Docker' },
              { id: 'postmortem', icon: <Clock size={11} />,     label: 'Postmortem' },
            ] as const
          ).map((t) => (
            <button
              key={t.id}
              className={`tab-btn ${mode === t.id ? 'active' : ''}`}
              onClick={() => setMode(t.id)}
            >
              {t.icon}
              {t.label}
            </button>
          ))}
        </div>

        <div className="spacer" />

        <div className="input-row" style={{ width: 200 }}>
          <Filter size={12} style={{ color: 'var(--text-muted)', flexShrink: 0 }} />
          <input
            type="text"
            placeholder="Filter..."
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
          />
        </div>
      </div>

      <div
        ref={bodyRef}
        className="card-body custom-scrollbar"
        onScroll={handleScroll}
      >
        {renderContent()}
      </div>

      <div className="card-footer" style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ display: 'flex', gap: '0.75rem', alignItems: 'center' }}>
          <span
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: '0.35rem',
              fontWeight: 600,
            }}
          >
            <span
              style={{
                width: 6, height: 6, borderRadius: '50%',
                background: 'var(--accent-green)',
                boxShadow: '0 0 6px rgba(34,197,94,0.5)',
                display: 'inline-block',
              }}
              className="pulse"
            />
            Live
          </span>
          {mode === 'raw' && <span>{raw.length} lines</span>}
          {mode === 'compressed' && <span>{compressed?.clusters?.length ?? 0} clusters · {compressed?.stack_traces?.length ?? 0} traces</span>}
          {mode === 'docker' && <span>{docker?.containers?.length ?? 0} containers</span>}
          {mode === 'postmortem' && <span>{postmortem?.window_minutes ?? 30}m window</span>}
        </div>
        <span className="mono" style={{ color: 'var(--accent-cyan)', opacity: 0.5, fontSize: '0.62rem' }}>
          BlackBox · port 8768
        </span>
      </div>
    </div>
  );
};

const Loading = () => (
  <div style={{ height: '100%', minHeight: 200, display: 'flex', alignItems: 'center', justifyContent: 'center', gap: '0.75rem', color: 'var(--text-muted)' }}>
    <span style={{ fontSize: '0.7rem', textTransform: 'uppercase', letterSpacing: '0.1em' }}>Loading…</span>
  </div>
);

const Empty = ({ icon, message }: { icon: React.ReactNode; message: string }) => (
  <div style={{ height: '100%', minHeight: 200, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: '0.75rem', color: 'var(--text-muted)', padding: '2rem', textAlign: 'center' }}>
    <span style={{ opacity: 0.15 }}>{icon}</span>
    <p style={{ fontSize: '0.7rem', textTransform: 'uppercase', letterSpacing: '0.08em' }}>{message}</p>
  </div>
);

const SectionLabel = ({ icon, text, color }: { icon: React.ReactNode; text: string; color: string }) => (
  <div style={{ display: 'flex', alignItems: 'center', gap: '0.375rem' }}>
    <span style={{ color }}>{icon}</span>
    <span style={{ fontSize: '0.62rem', fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.1em', color: 'var(--text-muted)' }}>
      {text}
    </span>
  </div>
);
