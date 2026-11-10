import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
// The design kit's colours, fonts and sizes load before anything else
import './ui/tokens.css'
import './global.css'
import App from './App.tsx'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
