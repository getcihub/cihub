import type { Installation, Membership } from '@/types/installation';
import type { ReactNode } from 'react';
import { createContext } from 'react';

export interface InstallationContextType {
    selectedInstallation: Installation | null;
    membership: Membership | null;
    isLoading: boolean;
    selectInstallation: (installation: Installation) => Promise<void>;
    clearInstallation: () => void;
}

export const InstallationContext = createContext<
    InstallationContextType | undefined
>(undefined);

export interface InstallationProviderProps {
    children: ReactNode;
}
