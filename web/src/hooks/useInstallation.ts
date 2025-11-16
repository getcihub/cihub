import { InstallationContext } from '@/context/InstallationContext';
import { useContext } from 'react';

export function useInstallation() {
    const context = useContext(InstallationContext);
    if (!context) {
        throw new Error(
            'useInstallation must be used within an InstallationProvider',
        );
    }
    return context;
}
