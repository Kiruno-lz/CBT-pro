import { useState, ReactNode } from 'react';
import { Link, useLocation } from 'react-router-dom';
import { useAppStore } from '../stores/useAppStore';

interface LayoutProps {
  children: ReactNode;
  sidebar?: ReactNode;
  rightPanel?: ReactNode;
}

export default function Layout({ children, sidebar, rightPanel }: LayoutProps) {
  const wsConnected = useAppStore((s) => s.wsConnected);
  const engineOnline = useAppStore((s) => s.engineOnline);
  const [sidebarOpen, setSidebarOpen] = useState(true);
  const [rightOpen, setRightOpen] = useState(true);
  const location = useLocation();

  return (
    <div className="min-h-screen bg-slate-950 text-slate-200 flex flex-col">
      {/* Header */}
      <header className="h-12 border-b border-slate-800 bg-slate-900 flex items-center px-4 gap-4 shrink-0">
        <Link to="/" className="flex items-center gap-2">
          <div className="w-6 h-6 rounded bg-blue-600 flex items-center justify-center">
            <span className="text-xs font-bold text-white">C</span>
          </div>
          <span className="font-bold text-sm tracking-tight text-slate-100">CBT-Pro</span>
        </Link>

        <nav className="flex items-center gap-1 ml-4">
          {[
            { path: '/', label: 'Dashboard' },
            { path: '/config', label: 'Config' },
            { path: '/results', label: 'Results' },
          ].map((item) => (
            <Link
              key={item.path}
              to={item.path}
              className={`px-3 py-1 rounded text-xs font-medium transition-colors ${
                location.pathname === item.path
                  ? 'bg-slate-800 text-blue-400'
                  : 'text-slate-400 hover:text-slate-200 hover:bg-slate-800/50'
              }`}
            >
              {item.label}
            </Link>
          ))}
        </nav>

        <div className="ml-auto flex items-center gap-3">
          <div className="flex items-center gap-1.5">
            <div
              className={`w-2 h-2 rounded-full ${
                wsConnected ? 'bg-green-500 animate-pulse' : 'bg-red-500'
              }`}
            />
            <span className="text-[10px] text-slate-500 uppercase">
              {wsConnected ? 'WS' : 'Offline'}
            </span>
          </div>
          <div className="flex items-center gap-1.5">
            <div
              className={`w-2 h-2 rounded-full ${
                engineOnline ? 'bg-green-500' : 'bg-amber-500'
              }`}
            />
            <span className="text-[10px] text-slate-500 uppercase">
              {engineOnline ? 'Engine' : 'Idle'}
            </span>
          </div>
        </div>
      </header>

      {/* Main Content Area */}
      <div className="flex-1 flex overflow-hidden">
        {/* Left Sidebar */}
        {sidebar && (
          <aside
            className={`border-r border-slate-800 bg-slate-900 flex flex-col transition-all duration-200 ${
              sidebarOpen ? 'w-64' : 'w-0 overflow-hidden'
            }`}
          >
            <div className="flex-1 overflow-y-auto">{sidebar}</div>
          </aside>
        )}
        {sidebar && (
          <button
            onClick={() => setSidebarOpen(!sidebarOpen)}
            className="absolute left-0 top-1/2 -translate-y-1/2 z-20 w-4 h-12 bg-slate-800 hover:bg-slate-700 rounded-r flex items-center justify-center text-[10px] text-slate-400"
            style={{ marginLeft: sidebarOpen ? '16rem' : 0 }}
          >
            {sidebarOpen ? '◀' : '▶'}
          </button>
        )}

        {/* Main Content */}
        <main className="flex-1 flex flex-col overflow-hidden">{children}</main>

        {/* Right Panel */}
        {rightPanel && (
          <aside
            className={`border-l border-slate-800 bg-slate-900 flex flex-col transition-all duration-200 ${
              rightOpen ? 'w-72' : 'w-0 overflow-hidden'
            }`}
          >
            <div className="flex-1 overflow-y-auto">{rightPanel}</div>
          </aside>
        )}
        {rightPanel && (
          <button
            onClick={() => setRightOpen(!rightOpen)}
            className="absolute right-0 top-1/2 -translate-y-1/2 z-20 w-4 h-12 bg-slate-800 hover:bg-slate-700 rounded-l flex items-center justify-center text-[10px] text-slate-400"
            style={{ marginRight: rightOpen ? '18rem' : 0 }}
          >
            {rightOpen ? '▶' : '◀'}
          </button>
        )}
      </div>
    </div>
  );
}
