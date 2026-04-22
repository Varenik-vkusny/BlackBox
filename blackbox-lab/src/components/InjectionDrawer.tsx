import { useState } from 'react';
import { motion, AnimatePresence } from 'framer-motion';
import { InjectionStation } from './InjectionStation';

interface Props {
  onInject: (text: string) => void;
  onClear: () => void;
  isPaused: boolean;
  onTogglePause: () => void;
}

export function InjectionDrawer({ onInject, onClear, isPaused, onTogglePause }: Props) {
  const [open, setOpen] = useState(false);

  return (
    <div>
      {/* Toggle bar */}
      <button
        className="inject-drawer-toggle"
        onClick={() => setOpen(o => !o)}
        aria-expanded={open}
        aria-label="Toggle injection console"
      >
        <svg
          width="10"
          height="10"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="2.5"
          style={{ transform: open ? 'rotate(180deg)' : 'none', transition: 'transform 0.2s ease' }}
        >
          <polyline points="18 15 12 9 6 15" />
        </svg>
        <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5">
          <polyline points="4 17 10 11 4 5" /><line x1="12" y1="19" x2="20" y2="19" />
        </svg>
        Inject  ·  <span style={{ fontVariantNumeric: 'tabular-nums' }}>` </span> to toggle
      </button>

      {/* Collapsible content */}
      <AnimatePresence>
        {open && (
          <motion.div
            className="inject-drawer-content"
            initial={{ height: 0, opacity: 0 }}
            animate={{ height: 320, opacity: 1 }}
            exit={{ height: 0, opacity: 0 }}
            transition={{ type: 'spring', stiffness: 300, damping: 30 }}
            style={{ overflow: 'hidden' }}
          >
            <InjectionStation
              onInject={onInject}
              onClear={onClear}
              isPaused={isPaused}
              onTogglePause={onTogglePause}
            />
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
