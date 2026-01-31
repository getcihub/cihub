import { createRoot } from 'react-dom/client';
import { QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider } from '@tanstack/react-router';
import { StrictMode, Suspense } from 'react';
import { Toaster } from 'sonner';

import { queryClient } from './lib/queryClient';
import { router } from './router';

import './index.css';

// import { QueryProvider } from './providers/QueryProvider';
// import { router } from './router.tsx';

createRoot(document.getElementById('root')!).render(
    <StrictMode>
        <Suspense fallback={null}>
            <QueryClientProvider client={queryClient}>
                <RouterProvider router={router} />
            </QueryClientProvider>
            <Toaster />
        </Suspense>
    </StrictMode>,
);
