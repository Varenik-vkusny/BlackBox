import { useState, useEffect, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { StatusBar } from './components/StatusBar';
import { SourceMatrix } from './components/SourceMatrix';
import { OverviewDashboard } from './components/OverviewDashboard';
import { TriageView } from './components/TriageView';
import { UnifiedStream } from './components/UnifiedStream';
import { RawLogsView } from './components/RawLogsView';
import { CorrelationPanel } from './components/CorrelationPanel';
import { InjectionDrawer } from './components/InjectionDrawer';
import { useDaemon } from './hooks/useDaemon';

export type DashboardView = 'overview' | 'triage' | 'raw';

function App() {
  const daemon = useDaemon();
  const [view, setView] = useState<DashboardView>('overview');
  const [triageService, setTriageService] = useState<string | null>(null);

  const navigateTriage = useCallback((service: string) => {
    setTriageService(service);
    setView('triage');
  }, []);

  const navigateOverview = useCallback(() => {
    setView('overview');
    setTriageService(null);
    daemon.setSelectedSource(null);
  }, [daemon]);

  const navigateRaw = useCallback(() => {
    setView('raw');
    daemon.setSelectedSource(null);
  }, [daemon]);

  // Backtick toggles pause
  const handleKeyDown = useCallback((e: KeyboardEvent) => {
    if (e.key === '`' && !e.ctrlKey && !e.metaKey && !e.altKey) {
      const target = e.target as HTMLElement;
      if (target.tagName !== 'TEXTAREA' && target.tagName !== 'INPUT') {
        e.preventDefault();
        daemon.setIsPaused(p => !p);
      }
    }
    // Escape from triage → overview
    if (e.key === 'Escape' && view === 'triage') {
      navigateOverview();
    }
  }, [daemon, view, navigateOverview]);

  useEffect(() => {
    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [handleKeyDown]);

  return (
    <div className="flight-deck">
      {/* Zone 1: Status bar (48px) */}
      <div className="zone-timeline">
        <StatusBar
          status={daemon.status}
          docker={daemon.docker}
          httpErrors={daemon.httpErrors}
          daemonOnline={daemon.daemonOnline}
        />
      </div>

      {/* Zone 2: Source matrix sidebar */}
      <div className="zone-matrix">
        <SourceMatrix
          status={daemon.status}
          docker={daemon.docker}
          httpErrors={daemon.httpErrors}
          watched={daemon.watched}
          selectedSource={daemon.selectedSource}
          onSelectSource={daemon.setSelectedSource}
          currentView={view}
          triageService={triageService}
          onNavigateTriage={navigateTriage}
          onNavigateOverview={navigateOverview}
          onNavigateRaw={navigateRaw}
        />
      </div>

      {/* Zone 3: Main content — routed by view */}
      <div className="zone-stream">
        {view === 'overview' && (
          <OverviewDashboard
            status={daemon.status}
            compressed={daemon.compressed}
            docker={daemon.docker}
            httpErrors={daemon.httpErrors}
            postmortem={daemon.postmortem}
            commits={daemon.commits}
            watched={daemon.watched}
            logs={daemon.logs}
            daemonOnline={daemon.daemonOnline}
            onNavigateTriage={navigateTriage}
            onNavigateRaw={navigateRaw}
          />
        )}
        {view === 'triage' && (
          <TriageView
            service={triageService ?? 'terminal'}
            compressed={daemon.compressed}
            docker={daemon.docker}
            httpErrors={daemon.httpErrors}
            diff={daemon.diff}
            selectedSource={daemon.selectedSource}
            onBack={navigateOverview}
            onNavigateRaw={navigateRaw}
            onInspectDiff={daemon.refreshDiff}
          />
        )}
        {view === 'raw' && (
          <RawLogsView
            logs={daemon.logs}
            docker={daemon.docker}
            httpErrors={daemon.httpErrors}
            onBack={navigateOverview}
          />
        )}
      </div>

      {/* Zone 4: Injection drawer */}
      <div className="zone-drawer">
        <InjectionDrawer
          onInject={daemon.injectLog}
          onClear={daemon.clearLogs}
          isPaused={daemon.isPaused}
          onTogglePause={() => daemon.setIsPaused(p => !p)}
        />
      </div>

      {/* Correlation panel overlay */}
      {daemon.correlationTarget && createPortal(
        <CorrelationPanel
          target={daemon.correlationTarget}
          correlated={daemon.correlated}
          onClose={() => daemon.setCorrelationTarget(null)}
        />,
        document.body
      )}
    </div>
  );
}

export default App;
