import React from 'react';
import { Search, RefreshCw } from 'lucide-react';
import type { DiffResponse } from '../types';

interface GitLensProps {
  diff: DiffResponse | null;
  onRefresh: () => void;
  loading?: boolean;
}

export const GitLens: React.FC<GitLensProps> = ({ diff, onRefresh, loading }) => {
  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
      {/* Toolbar */}
      <div style={{
        display: 'flex', alignItems: 'center', gap: '8px',
        padding: '8px 12px', borderBottom: '1px solid var(--border-subtle)',
        flexShrink: 0, background: 'var(--bg-raised)',
      }}>
        <span style={{ flex: 1, fontSize: '11px', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.08em', color: 'var(--fg-muted)' }}>
          Contextual Diff
        </span>
        <button onClick={onRefresh} className="btn btn-ghost" disabled={loading}
          style={{ minHeight: 24, padding: '2px 8px', fontSize: '11px', fontFamily: "'Geist Mono', monospace" }}
        >
          <RefreshCw size={10} style={loading ? { animation: 'spin 1s linear infinite' } : undefined} />
          {loading ? 'Scanning…' : 'Scan Context'}
        </button>
      </div>

      <div className="custom-scrollbar" style={{ flex: 1, overflowY: 'auto', background: 'rgba(0,0,0,0.15)' }}>
        {!diff ? (
          <div
            style={{
              height: '100%', minHeight: 200,
              display: 'flex', flexDirection: 'column',
              alignItems: 'center', justifyContent: 'center',
              gap: '12px', color: 'var(--fg-muted)',
              padding: '32px', textAlign: 'center',
            }}
          >
            <Search size={32} style={{ opacity: 0.12 }} />
            <p style={{ fontSize: '11px', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.08em' }}>
              No active context
            </p>
            <p style={{ fontSize: '11px', color: 'var(--fg-muted)', opacity: 0.6, lineHeight: 1.6, maxWidth: 220 }}>
              Cross-references error stack files with dirty git hunks
            </p>
          </div>
        ) : (
          <div style={{ padding: '12px' }}>
            {diff.diff_hunks.length === 0 && (
              <div
                style={{
                  padding: '32px', textAlign: 'center',
                  color: 'var(--fg-muted)', fontSize: '12px', fontStyle: 'italic',
                }}
              >
                No changed lines intersected with recent error files
              </div>
            )}

            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
              {diff.diff_hunks.map((hunk, i) => (
                <div
                  key={i}
                  style={{
                    borderRadius: 'var(--radius-sm)',
                    overflow: 'hidden',
                    border: '1px solid var(--border-subtle)',
                  }}
                >
                  <div
                    style={{
                      padding: '6px 12px',
                      background: 'var(--bg-raised)',
                      borderBottom: '1px solid var(--border-subtle)',
                      display: 'flex',
                      alignItems: 'center',
                      gap: '6px',
                    }}
                  >
                    <span className="mono truncate" style={{ fontSize: '12px', color: 'var(--fg-secondary)', fontWeight: 500 }}>
                      {hunk.file}
                    </span>
                    <span className="mono" style={{ fontSize: '11px', color: 'var(--fg-muted)', marginLeft: 'auto', flexShrink: 0 }}>
                      @{hunk.new_start}
                    </span>
                  </div>
                  <div style={{ padding: '4px', background: 'rgba(0,0,0,0.25)' }}>
                    {hunk.lines.map((line, li) => {
                      const bg =
                        line.kind === 'added' ? '#1e4429' :
                        line.kind === 'removed' ? '#3a1c1c' :
                        'transparent';
                      const color =
                        line.kind === 'added' ? '#86efac' :
                        line.kind === 'removed' ? '#fca5a5' :
                        'var(--fg-muted)';
                      return (
                        <div
                          key={li}
                          className="mono"
                          style={{
                            padding: '2px 8px',
                            borderRadius: '2px',
                            fontSize: '12px',
                            lineHeight: 1.65,
                            whiteSpace: 'pre',
                            background: bg,
                            color: color,
                          }}
                        >
                          {line.kind === 'added' ? '+' : line.kind === 'removed' ? '-' : ' '}
                          {line.text}
                        </div>
                      );
                    })}
                  </div>
                </div>
              ))}
            </div>

            {diff.files_cross_referenced.length > 0 && (
              <div style={{ marginTop: '16px' }}>
                <p style={{
                  fontSize: '11px', fontWeight: 600,
                  textTransform: 'uppercase', letterSpacing: '0.08em',
                  color: 'var(--fg-muted)', marginBottom: '8px',
                }}
                >
                  Referenced files
                </p>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px' }}>
                  {diff.files_cross_referenced.map((f) => (
                    <span
                      key={f}
                      className="mono"
                      style={{
                        padding: '2px 8px',
                        borderRadius: '4px',
                        background: 'var(--bg-raised)',
                        border: '1px solid var(--border-subtle)',
                        fontSize: '11px',
                        color: 'var(--fg-secondary)',
                      }}
                    >
                      {f}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {diff.truncated && (
              <p style={{ marginTop: '12px', fontSize: '11px', color: 'var(--severity-warn)', opacity: 0.7 }}
              >
                Diff truncated — too many hunks
              </p>
            )}
          </div>
        )}
      </div>

      <div style={{
        display: 'flex', alignItems: 'center', gap: '6px',
        padding: '4px 12px', borderTop: '1px solid var(--border-subtle)',
        background: 'var(--bg-raised)', flexShrink: 0,
        fontSize: '11px', color: 'var(--fg-muted)',
      }}>
        <span>error files ∩ dirty git files</span>
        {diff && (
          <span style={{ marginLeft: 'auto', color: 'var(--fg-muted)', opacity: 0.6 }}>
            {diff.diff_hunks.length} hunks
          </span>
        )}
      </div>
    </div>
  );
};
