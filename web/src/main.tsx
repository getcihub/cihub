import { RouterProvider } from '@tanstack/react-router';
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';

import './index.css';
import { QueryProvider } from './providers/QueryProvider';
import { router } from './router.tsx';

createRoot(document.getElementById('root')!).render(
    <StrictMode>
        <QueryProvider>
            <RouterProvider router={router} />
        </QueryProvider>
    </StrictMode>,
);
