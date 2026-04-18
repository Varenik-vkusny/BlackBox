import React from 'react';
import { Activity, GitBranch, Terminal, Cpu, Clock, FileDiff } from 'lucide-react';
import type { BBStatus } from '../types';

interface HeaderProps {
  status: BBStatus | null;
  daemonOnline: boolean;
}

function formatUptime(secs: number): string {
  if (secs < 60) return `${secs}s`;
  if (secs < 3600) return `${Math.floor(secs / 60)}m ${secs % 60}s`;
  return `${Math.floor(secs / 3600)}h ${Math.floor((secs % 3600) / 60)}m`;
}

export const Header: React.FC<HeaderProps> = ({ status, daemonOnline }) => {
  return (
    <header
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        gap: '1rem',
        flexWrap: 'wrap',
        padding: '0.25rem 0',
      }}
    >
      {/* Brand */}
      <div style={{ display: 'flex', alignItems: 'center', gap: '0.75rem' }}>
        <div
          style={{
            width: 40,
            height: 40,
            borderRadius: '0.5rem',
            border: '1px solid rgba(34,211,238,0.2)',
            background: 'rgba(34,211,238,0.06)',
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'center',
            flexShrink: 0,
          }}
        >
          <Activity size={18} style={{ color: 'var(--accent-cyan)' }} />
        </div>
        <div>
          <h1
            style={{
              fontFamily: "'JetBrains Mono', monospace",
              fontSize: '1.25rem',
              fontWeight: 700,
              letterSpacing: '-0.02em',
              lineHeight: 1.2,
            }}
          >
            BlackBox<span style={{ color: 'var(--accent-cyan)' }}>Lab</span>
          </h1>
          <p
            style={{
              fontSize: '0.6rem',
              fontWeight: 600,
              textTransform: 'uppercase',
              letterSpacing: '0.18em',
              color: 'var(--text-muted)',
              marginTop: 2,
            }}
          >
            Daemon Control · Context Debugger
          </p>
        </div>
      </div>

      {/* Stats */}
      <div style={{ display: 'flex', gap: '0.5rem', flexWrap: 'wrap', alignItems: 'center' }}>
        <Stat icon={<Terminal size={11} />} label="Buffer" value={`${status?.buffer_lines ?? 0} lines`} />
        <Stat icon={<GitBranch size={11} />} label="Branch" value={status?.git_branch ?? 'detached'} />
        <Stat icon={<Cpu size={11} />} label="Project" value={status?.project_type ?? '–'} />
        <Stat icon={<FileDiff size={11} />} label="Dirty" value={`${status?.git_dirty_files ?? 0} files`} />
        <Stat icon={<Clock size={11} />} label="Uptime" value={status ? formatUptime(status.uptime_secs) : '–'} />

        {/* Status pill */}
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: '0.4rem',
            padding: '0.4rem 0.875rem',
            borderRadius: '9999px',
            border: `1px solid ${!daemonOnline ? 'rgba(71,85,105,0.4)' : status?.has_recent_errors ? 'rgba(244,63,94,0.3)' : 'rgba(34,197,94,0.3)'}`,
            background: !daemonOnline ? 'rgba(71,85,105,0.08)' : status?.has_recent_errors ? 'rgba(244,63,94,0.07)' : 'rgba(34,197,94,0.07)',
          }}
        >
          <span
            style={{
              width: 7,
              height: 7,
              borderRadius: '50%',
              background: !daemonOnline ? 'var(--text-muted)' : status?.has_recent_errors ? 'var(--accent-red)' : 'var(--accent-green)',
              boxShadow: daemonOnline && !status?.has_recent_errors ? '0 0 8px rgba(34,197,94,0.6)' : 'none',
              flexShrink: 0,
            }}
            className={daemonOnline ? 'pulse' : undefined}
          />
          <span
            style={{
              fontSize: '0.62rem',
              fontWeight: 700,
              textTransform: 'uppercase',
              letterSpacing: '0.1em',
              color: !daemonOnline ? 'var(--text-muted)' : status?.has_recent_errors ? 'var(--accent-red)' : 'var(--accent-green)',
            }}
          >
            {!daemonOnline ? 'Offline' : status?.has_recent_errors ? 'Errors Detected' : 'Nominal'}
          </span>
        </div>
      </div>
    </header>
  );
};

const Stat: React.FC<{ icon: React.ReactNode; label: string; value: string }> = ({ icon, label, value }) => (
  <div
    style={{
      display: 'flex',
      flexDirection: 'column',
      padding: '0.4rem 0.75rem',
      borderRadius: '0.4rem',
      border: '1px solid var(--border)',
      background: 'rgba(0,0,0,0.2)',
      minWidth: 80,
    }}
  >
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: '0.25rem',
        color: 'var(--text-muted)',
        marginBottom: '0.15rem',
      }}
    >
      {icon}
      <span style={{ fontSize: '0.58rem', fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.1em' }}>
        {label}
      </span>
    </div>
    <span
      className="mono truncate"
      style={{ fontSize: '0.7rem', fontWeight: 600, color: 'var(--text-primary)', maxWidth: 120 }}
    >
      {value}
    </span>
  </div>
);
