import { useEffect, useCallback } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import type { StreamEntry, CorrelatedResponse } from '../types';

interface Props {
  target: StreamEntry;
  correlated: CorrelatedResponse | null;
  onClose: () => void;
}

function timeLabel(ms: number): string {
  return new Date(ms).toTimeString().slice(0, 8);
}

function relativeMs(a: number, b: number): string {
  const d = b - a;
  if (d === 0) return 'same time';
  const sign = d > 0 ? '+' : '−';
  return `${sign}${Math.abs(Math.round(d / 1000))}s`;
}

export function CorrelationPanel({ target, correlated, onClose }: Props) {
  // Close on Escape
  const handleKey = useCallback((e: KeyboardEvent) => {
    if (e.key === 'Escape') onClose();
  }, [onClose]);

  useEffect(() => {
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, [handleKey]);

  // Find correlations near target timestamp
  const nearby = correlated?.correlations?.filter(c => {
    const d = Math.abs(c.timestamp_ms - target.timestamp_ms);
    return d <= 10000; // ±10s window
  }) ?? [];

  const dockerEvents = nearby.flatMap(c => c.correlated_docker_events ?? []);
  const httpEvents   = nearby.flatMap(c => (c as any).correlated_http_errors ?? []);
  const hasAny       = dockerEvents.length > 0 || httpEvents.length > 0;

  return (
    <AnimatePresence>
      {/* Backdrop */}
      <motion.div
        className="correlation-overlay"
        initial={{ opacity: 0 }}
        animate={{ opacity: 1 }}
        exit={{ opacity: 0 }}
        transition={{ duration: 0.15 }}
        onClick={onClose}
      />

      {/* Panel */}
      <motion.div
        className="correlation-panel"
        initial={{ x: '100%' }}
        animate={{ x: 0 }}
        exit={{ x: '100%' }}
        transition={{ type: 'spring', stiffness: 400, damping: 35 }}
      >
        <div className="correlation-header">
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="var(--accent-cyan)" strokeWidth="2.5">
            <circle cx="18" cy="18" r="3" /><circle cx="6" cy="6" r="3" /><circle cx="6" cy="18" r="3" />
            <line x1="6" y1="9" x2="6" y2="15" /><line x1="15.24" y1="6.75" x2="8.76" y2="17.25" />
          </svg>
          <span className="correlation-title">Correlated Events</span>
          <span style={{ fontSize: '0.6rem', color: 'var(--text-muted)', fontFamily: "'JetBrains Mono', monospace" }}>
            ±10s window
          </span>
          <button
            className="btn btn-ghost"
            style={{ padding: '0.2rem 0.45rem', minHeight: 28, marginLeft: 'auto' }}
            onClick={onClose}
            aria-label="Close correlation panel"
          >
            ✕
          </button>
        </div>

        {/* Trigger line */}
        <div className="correlation-trigger-line">
          <span style={{ color: 'var(--text-muted)', marginRight: '0.5rem' }}>{timeLabel(target.timestamp_ms)}</span>
          {target.text}
        </div>

        <div className="correlation-body custom-scrollbar">
          {!hasAny ? (
            <div className="correlation-empty">
              <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" style={{ opacity: 0.3 }}>
                <circle cx="12" cy="12" r="9" /><line x1="9" y1="9" x2="9.01" y2="9" /><line x1="15" y1="9" x2="15.01" y2="9" />
                <path d="M9.09 14.5s.91 1.5 2.91 1.5 2.91-1.5 2.91-1.5" />
              </svg>
              <span>No correlated events in ±10s</span>
              <span style={{ fontSize: '0.65rem', color: 'var(--text-muted)' }}>
                Try clicking a log line closer to a known error spike
              </span>
            </div>
          ) : (
            <>
              {dockerEvents.length > 0 && (
                <>
                  <div className="correlation-section-label">Docker Events</div>
                  {dockerEvents.map((ev, i) => (
                    <div key={i} className={`correlation-event ${ev.level ?? ''}`}>
                      <div className="correlation-event-text">{ev.text}</div>
                      <div className="correlation-event-meta">
                        <span>{typeof ev.source === 'string' ? ev.source.slice(0, 12) : 'docker'}</span>
                        <span style={{ color: 'var(--text-muted)', marginLeft: 'auto' }}>
                          {relativeMs(target.timestamp_ms, nearby[0]?.timestamp_ms ?? target.timestamp_ms)}
                        </span>
                      </div>
                    </div>
                  ))}
                </>
              )}

              {httpEvents.length > 0 && (
                <>
                  <div className="correlation-section-label" style={{ marginTop: '0.75rem' }}>HTTP Errors</div>
                  {httpEvents.map((ev: any, i: number) => (
                    <div key={i} className="correlation-event error">
                      <div className="correlation-event-text">
                        <span style={{ color: ev.status >= 500 ? 'var(--accent-red)' : 'var(--accent-orange)', fontWeight: 700 }}>
                          {ev.status}
                        </span>
                        {' '}{ev.method} {ev.url}
                      </div>
                      <div className="correlation-event-meta">
                        <span>{ev.latency_ms}ms</span>
                        <span style={{ color: 'var(--text-muted)', marginLeft: 'auto' }}>
                          {relativeMs(target.timestamp_ms, ev.timestamp_ms ?? target.timestamp_ms)}
                        </span>
                      </div>
                    </div>
                  ))}
                </>
              )}
            </>
          )}
        </div>
      </motion.div>
    </AnimatePresence>
  );
}
