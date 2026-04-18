import React from 'react';
import { FileDiff, GitCommit, Search, ChevronRight, RefreshCw } from 'lucide-react';
import type { DiffResponse } from '../types';

interface GitLensProps {
  diff: DiffResponse | null;
  onRefresh: () => void;
  loading?: boolean;
}

export const GitLens: React.FC<GitLensProps> = ({ diff, onRefresh, loading }) => {
  return (
    <div className="card" style={{ height: 700 }}>
      <div className="card-header">
        <FileDiff size={14} style={{ color: 'var(--accent-red)', flexShrink: 0 }} />
        <span className="card-title">Contextual Diff</span>
        <div className="spacer" />
        <button onClick={onRefresh} className="btn btn-primary" disabled={loading}>
          <RefreshCw size={11} style={loading ? { animation: 'spin 1s linear infinite' } : undefined} />
          {loading ? 'Scanning…' : 'Scan Context'}
        </button>
      </div>

      <div className="card-body custom-scrollbar" style={{ background: 'rgba(0,0,0,0.15)' }}>
        {!diff ? (
          <div
            style={{
              height: '100%',
              minHeight: 200,
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              justifyContent: 'center',
              gap: '0.75rem',
              color: 'var(--text-muted)',
              padding: '2rem',
              textAlign: 'center',
            }}
          >
            <Search size={32} style={{ opacity: 0.12 }} />
            <p style={{ fontSize: '0.7rem', textTransform: 'uppercase', letterSpacing: '0.08em' }}>
              No active context
            </p>
            <p style={{ fontSize: '0.65rem', color: 'var(--text-muted)', opacity: 0.6, lineHeight: 1.6, maxWidth: 220 }}>
              Cross-references error stack files with dirty git hunks
            </p>
          </div>
        ) : (
          <div style={{ padding: '0.875rem' }}>
            {diff.diff_hunks.length === 0 && (
              <div
                style={{
                  padding: '2rem',
                  textAlign: 'center',
                  color: 'var(--text-muted)',
                  fontSize: '0.68rem',
                  fontStyle: 'italic',
                }}
              >
                No changed lines intersected with recent error files
              </div>
            )}

            <div style={{ display: 'flex', flexDirection: 'column', gap: '0.75rem' }}>
              {diff.diff_hunks.map((hunk, i) => (
                <div
                  key={i}
                  style={{
                    borderRadius: '0.4rem',
                    overflow: 'hidden',
                    border: '1px solid var(--border)',
                  }}
                >
                  <div
                    style={{
                      padding: '0.4rem 0.75rem',
                      background: 'rgba(34,211,238,0.05)',
                      borderBottom: '1px solid var(--border)',
                      display: 'flex',
                      alignItems: 'center',
                      gap: '0.375rem',
                    }}
                  >
                    <ChevronRight size={11} style={{ color: 'var(--accent-cyan)', flexShrink: 0 }} />
                    <span
                      className="mono truncate"
                      style={{ fontSize: '0.65rem', color: 'var(--accent-cyan)', fontWeight: 600 }}
                    >
                      {hunk.file}
                    </span>
                    <span className="mono" style={{ fontSize: '0.6rem', color: 'var(--text-muted)', marginLeft: 'auto', flexShrink: 0 }}>
                      @{hunk.new_start}
                    </span>
                  </div>
                  <div
                    style={{
                      padding: '0.375rem',
                      background: 'rgba(0,0,0,0.25)',
                    }}
                  >
                    {hunk.lines.map((line, li) => (
                      <div
                        key={li}
                        className="mono"
                        style={{
                          padding: '0.1rem 0.5rem',
                          borderRadius: 3,
                          fontSize: '0.65rem',
                          lineHeight: 1.65,
                          whiteSpace: 'pre',
                          background:
                            line.kind === 'added' ? 'rgba(34,197,94,0.08)' :
                            line.kind === 'removed' ? 'rgba(244,63,94,0.08)' :
                            'transparent',
                          color:
                            line.kind === 'added' ? 'var(--accent-green)' :
                            line.kind === 'removed' ? 'var(--accent-red)' :
                            'var(--text-muted)',
                        }}
                      >
                        {line.kind === 'added' ? '+' : line.kind === 'removed' ? '-' : ' '}
                        {line.text}
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>

            {diff.files_cross_referenced.length > 0 && (
              <div style={{ marginTop: '1rem' }}>
                <p
                  style={{
                    fontSize: '0.6rem',
                    fontWeight: 700,
                    textTransform: 'uppercase',
                    letterSpacing: '0.1em',
                    color: 'var(--text-muted)',
                    marginBottom: '0.5rem',
                  }}
                >
                  Referenced files
                </p>
                <div style={{ display: 'flex', flexWrap: 'wrap', gap: '0.375rem' }}>
                  {diff.files_cross_referenced.map((f) => (
                    <span
                      key={f}
                      className="mono"
                      style={{
                        padding: '0.15rem 0.5rem',
                        borderRadius: '9999px',
                        background: 'rgba(255,255,255,0.04)',
                        border: '1px solid var(--border)',
                        fontSize: '0.62rem',
                        color: 'var(--text-secondary)',
                      }}
                    >
                      {f}
                    </span>
                  ))}
                </div>
              </div>
            )}

            {diff.truncated && (
              <p
                style={{
                  marginTop: '0.75rem',
                  fontSize: '0.62rem',
                  color: 'var(--accent-orange)',
                  opacity: 0.7,
                }}
              >
                ⚠ Diff truncated — too many hunks
              </p>
            )}
          </div>
        )}
      </div>

      <div className="card-footer" style={{ display: 'flex', alignItems: 'center', gap: '0.375rem' }}>
        <GitCommit size={11} />
        <span>error files ∩ dirty git files</span>
        {diff && (
          <span style={{ marginLeft: 'auto', color: 'var(--accent-cyan)', opacity: 0.6 }}>
            {diff.diff_hunks.length} hunks
          </span>
        )}
      </div>
    </div>
  );
};
