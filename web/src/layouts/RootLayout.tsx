import { Outlet } from '@tanstack/react-router';

export function RootLayout() {
    return (
        <div className="min-h-full bg-white antialiased">
            <Outlet />
        </div>
    );
}
