import { Outlet } from '@tanstack/react-router';
import { Toaster } from 'sonner';

import { AuthProvider } from './providers/AuthProvider';
import { InstallationProvider } from './providers/InstallationProvider';
import { TimeRangeProvider } from './context/TimeRangeContext';

function App() {
    return (
        <AuthProvider>
            <InstallationProvider>
                <TimeRangeProvider>
                    <Outlet />
                    <Toaster />
                </TimeRangeProvider>
            </InstallationProvider>
        </AuthProvider>
    );
}

export default App;
