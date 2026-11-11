import { HomeScreen } from './ui'

// The app's root: picks the screen. For now there's only home
function App() {
  // Create room and Join room are wired up in the next steps
  return <HomeScreen onCreate={() => {}} onJoin={() => {}} />
}

export default App
