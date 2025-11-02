import { Outlet } from '@tanstack/react-router'
import { AuthProvider } from './providers/AuthProvider'
import { InstallationProvider } from './providers/InstallationProvider'

function App() {
    return (
        <AuthProvider>
            <InstallationProvider>
                <Outlet />
            </InstallationProvider>
        </AuthProvider>
    )
}

export default App
