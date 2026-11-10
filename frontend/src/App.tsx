import { useServer } from './useServer'

// The app's root. Every screen comes from the design kit, wired up in later steps
function App() {
  const { status } = useServer()
  // Until the home screen, a way to see the connection works
  return <h1>rustyparty: {status}</h1>
}

export default App
