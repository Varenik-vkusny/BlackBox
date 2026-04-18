import { Header } from './components/Header';
import { LogExplorer } from './components/LogExplorer';
import { GitLens } from './components/GitLens';
import { InjectionStation } from './components/InjectionStation';
import { useDaemon } from './hooks/useDaemon';

function App() {
  const {
    status,
    logs,
    compressed,
    docker,
    diff,
    postmortem,
    correlated,
    loading,
    isPaused,
    daemonOnline,
    setIsPaused,
    refreshDiff,
    injectLog,
    clearLogs,
  } = useDaemon();

  return (
    <div className="animate-in">
      <Header status={status} daemonOnline={daemonOnline} />

      <main className="dashboard-grid">
        <div className="col-full">
          <InjectionStation
            onInject={injectLog}
            onClear={clearLogs}
            isPaused={isPaused}
            onTogglePause={() => setIsPaused(!isPaused)}
          />
        </div>

        <div className="col-8">
          <LogExplorer
            raw={logs}
            compressed={compressed}
            docker={docker}
            postmortem={postmortem}
            correlated={correlated}
          />
        </div>

        <div className="col-4">
          <GitLens diff={diff} onRefresh={refreshDiff} loading={loading} />
        </div>
      </main>

      <footer
        style={{
          padding: '1.5rem 0',
          textAlign: 'center',
          fontSize: '0.6rem',
          fontWeight: 600,
          textTransform: 'uppercase',
          letterSpacing: '0.2em',
          color: 'var(--text-muted)',
          opacity: 0.4,
          fontFamily: "'JetBrains Mono', monospace",
        }}
      >
        BlackBox Context Intelligence · Internal Lab · v3.0
      </footer>
    </div>
  );
}

export default App;
