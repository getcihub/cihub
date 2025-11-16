import { queryClient } from '@/lib/queryClient';
import { QueryClientProvider } from '@tanstack/react-query';

interface QueryProviderProps {
    children: React.ReactNode;
}

export function QueryProvider({ children }: QueryProviderProps) {
    return (
        <QueryClientProvider client={queryClient}>
            {children}
        </QueryClientProvider>
    );
}
