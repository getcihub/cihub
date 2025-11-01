import { Outlet } from '@tanstack/react-router'
import { AuthProvider } from './providers/AuthProvider'

function App() {
    return (
        <AuthProvider>
            <Outlet />
        </AuthProvider>
    )
}

export default App
