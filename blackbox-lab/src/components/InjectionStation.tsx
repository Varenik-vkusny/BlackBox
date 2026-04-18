import React, { useState } from 'react';
import { Send, Trash2, Pause, Play, Terminal, Shield, Layers, AlertTriangle } from 'lucide-react';

interface InjectionStationProps {
  onInject: (text: string) => void;
  onClear: () => void;
  isPaused: boolean;
  onTogglePause: () => void;
}

type Category = { label: string; icon: React.ReactNode; color: string; scenarios: Scenario[] };
type Scenario = { label: string; content: string };

const CATEGORIES: Category[] = [
  {
    label: 'Stack Traces',
    icon: <AlertTriangle size={12} />,
    color: 'var(--accent-red)',
    scenarios: [
      {
        label: 'Rust Panic',
        content: "thread 'main' panicked at 'index out of bounds: the len is 3 but the index is 5', src/main.rs:42:15\nstack backtrace:\n   0: rust_begin_unwind\n   1: core::panicking::panic_fmt\n   2: myapp::handler::process\n             at src/handler.rs:88:5\n   3: myapp::main\n             at src/main.rs:42:5",
      },
      {
        label: 'Node.js TypeError',
        content: "TypeError: Cannot read properties of undefined (reading 'map')\n    at processItems (src/utils.js:34:15)",
      },
      {
        label: 'Python Traceback',
        content: "Traceback (most recent call last):\n  File \"app/server.py\", line 88, in handle_request\n    result = db.query(sql)\n  File \"app/db.py\", line 42, in query\n    raise ConnectionError(f\"DB timeout: {host}\")\nConnectionError: DB timeout: 10.0.0.5",
      },
      {
        label: 'Java Exception',
        content: "java.lang.NullPointerException: Cannot invoke method handle()\n\tat com.example.api.Controller.process(Controller.java:77)\n\tat com.example.api.Router.dispatch(Router.java:31)",
      },
    ],
  },
  {
    label: 'PII Masking',
    icon: <Shield size={12} />,
    color: 'var(--accent-green)',
    scenarios: [
      {
        label: 'Email',
        content: 'User john.doe@company.com authenticated from 192.168.1.100',
      },
      {
        label: 'Bearer Token',
        content: 'Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiJ1c3IxMjMifQ.SflKxwRJSMeKKF2QT4fwpMeJf36POkX',
      },
      {
        label: 'Password Field',
        content: 'Connecting: host=db.internal port=5432 user=admin password=Sup3rS3cr3tP@ss! dbname=prod',
      },
      {
        label: 'AWS AKIA Key',
        content: 'AWS credentials loaded: AKIAIOSFODNN7EXAMPLE / wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY',
      },
      {
        label: 'High-Entropy Token',
        content: 'stripe_secret=sk-live-mNp8Qr2Xv5Kj9Lz4Wy7AsDk6vTrBn1Qz',
      },
      {
        label: 'Credit Card',
        content: 'Payment processed for card 4532015112830366 amount=$99.00 status=ok',
      },
    ],
  },
  {
    label: 'Edge Cases',
    icon: <Layers size={12} />,
    color: 'var(--accent-cyan)',
    scenarios: [
      {
        label: 'ANSI Colors',
        content: '\x1b[31mERROR\x1b[0m: \x1b[33mconnection refused\x1b[0m at \x1b[36m127.0.0.1:5432\x1b[0m (stripped by daemon)',
      },
      {
        label: 'Drain Cluster ×1',
        content: 'WARN: retry attempt 1/5 for endpoint /api/health (timeout=5000ms)',
      },
      {
        label: 'Drain Cluster ×2',
        content: 'WARN: retry attempt 2/5 for endpoint /api/health (timeout=5000ms)',
      },
      {
        label: 'Shell Hook (bash)',
        content: '$ cargo test --workspace --no-fail-fast 2>&1 | tee /tmp/test.log',
      },
    ],
  },
];

export const InjectionStation: React.FC<InjectionStationProps> = ({
  onInject,
  onClear,
  isPaused,
  onTogglePause,
}) => {
  const [customLog, setCustomLog] = useState('');
  const [activeCategory, setActiveCategory] = useState(0);
  const [lastInjected, setLastInjected] = useState<string | null>(null);

  const inject = (content: string) => {
    onInject(content);
    setLastInjected(content.split('\n')[0].slice(0, 60));
    setTimeout(() => setLastInjected(null), 2000);
  };

  const current = CATEGORIES[activeCategory];

  return (
    <div className="card">
      <div className="card-header">
        <Terminal size={14} style={{ color: 'var(--accent-cyan)', flexShrink: 0 }} />
        <span className="card-title">Injection Console</span>
        <div className="spacer" />
        <button
          onClick={onTogglePause}
          className={`btn ${isPaused ? 'btn-orange' : 'btn-ghost'}`}
        >
          {isPaused ? <Play size={12} /> : <Pause size={12} />}
          {isPaused ? 'Resume' : 'Pause'}
        </button>
        <button onClick={onClear} className="btn btn-red">
          <Trash2 size={12} />
          Clear Buffer
        </button>
      </div>

      <div className="p-body" style={{ display: 'flex', gap: '1.5rem', alignItems: 'flex-start' }}>
        {/* Left: scenario grid */}
        <div style={{ flex: '1 1 0', minWidth: 0 }}>
          {/* Category tabs */}
          <div style={{ display: 'flex', gap: '0.375rem', marginBottom: '0.875rem' }}>
            {CATEGORIES.map((cat, i) => (
              <button
                key={cat.label}
                onClick={() => setActiveCategory(i)}
                className={`tab-btn ${activeCategory === i ? 'active' : ''}`}
                style={activeCategory === i ? { color: cat.color } : {}}
              >
                <span style={{ color: activeCategory === i ? cat.color : 'var(--text-muted)' }}>
                  {cat.icon}
                </span>
                {cat.label}
              </button>
            ))}
          </div>

          {/* Scenario buttons */}
          <div
            style={{
              display: 'grid',
              gridTemplateColumns: 'repeat(auto-fill, minmax(180px, 1fr))',
              gap: '0.5rem',
            }}
          >
            {current.scenarios.map((s) => (
              <button
                key={s.label}
                onClick={() => inject(s.content)}
                className="btn btn-ghost"
                style={{
                  justifyContent: 'flex-start',
                  border: '1px solid var(--border)',
                  background: 'rgba(0,0,0,0.2)',
                  fontFamily: "'Inter', sans-serif",
                  textTransform: 'none',
                  letterSpacing: 'normal',
                  fontSize: '0.72rem',
                }}
              >
                <span style={{ width: 6, height: 6, borderRadius: '50%', background: current.color, flexShrink: 0 }} />
                {s.label}
              </button>
            ))}
          </div>
        </div>

        {/* Right: manual input */}
        <div style={{ width: 320, flexShrink: 0, display: 'flex', flexDirection: 'column', gap: '0.5rem' }}>
          {/* Header row */}
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
            <p style={{ fontSize: '0.63rem', fontWeight: 700, textTransform: 'uppercase', letterSpacing: '0.1em', color: 'var(--text-muted)' }}>
              Custom Payload
            </p>
            {customLog.trim() && (
              <span className="mono" style={{ fontSize: '0.6rem', color: 'var(--text-muted)' }}>
                {customLog.split('\n').filter(l => l.trim()).length} lines
              </span>
            )}
          </div>

          {/* Textarea */}
          <textarea
            placeholder={"Paste any log block here…\nSupports multiline (Ctrl+Enter to inject)"}
            value={customLog}
            onChange={(e) => setCustomLog(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && (e.ctrlKey || e.metaKey) && customLog.trim()) {
                inject(customLog);
                setCustomLog('');
              }
            }}
            spellCheck={false}
            style={{
              background: 'rgba(0,0,0,0.35)',
              border: '1px solid var(--border)',
              borderRadius: '0.4rem',
              color: 'var(--text-primary)',
              fontFamily: "'JetBrains Mono', monospace",
              fontSize: '0.7rem',
              lineHeight: 1.6,
              padding: '0.625rem 0.75rem',
              resize: 'vertical',
              minHeight: 120,
              maxHeight: 300,
              outline: 'none',
              width: '100%',
              transition: 'border-color 0.15s',
            }}
            onFocus={(e) => (e.currentTarget.style.borderColor = 'rgba(34,211,238,0.4)')}
            onBlur={(e) => (e.currentTarget.style.borderColor = 'var(--border)')}
          />

          <button
            onClick={() => {
              if (customLog.trim()) {
                inject(customLog);
                setCustomLog('');
              }
            }}
            className="btn btn-primary"
            style={{ width: '100%', justifyContent: 'center' }}
          >
            <Send size={12} />
            Inject
            {customLog.split('\n').filter(l => l.trim()).length > 1 && (
              <span style={{ opacity: 0.6, fontWeight: 400, textTransform: 'none', letterSpacing: 'normal' }}>
                ({customLog.split('\n').filter(l => l.trim()).length} lines)
              </span>
            )}
          </button>

          <p className="mono" style={{ fontSize: '0.58rem', color: 'var(--text-muted)', opacity: 0.6, lineHeight: 1.5 }}>
            Ctrl+Enter to inject · Each line is a separate LogLine
          </p>

          {lastInjected && (
            <p className="mono truncate" style={{ fontSize: '0.6rem', color: 'var(--accent-green)' }}>
              ✓ {lastInjected}
            </p>
          )}
        </div>
      </div>
    </div>
  );
};
