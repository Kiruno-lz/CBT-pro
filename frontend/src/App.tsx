import { Routes, Route } from 'react-router-dom'
import Dashboard from './pages/Dashboard'
import BacktestConfig from './pages/BacktestConfig'
import Results from './pages/Results'

function App() {
  return (
    <div className="min-h-screen bg-slate-950 text-slate-200">
      <Routes>
        <Route path="/" element={<Dashboard />} />
        <Route path="/config" element={<BacktestConfig />} />
        <Route path="/results" element={<Results />} />
      </Routes>
    </div>
  )
}

export default App
