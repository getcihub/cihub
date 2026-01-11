import { Outlet } from '@tanstack/react-router';

export function RootLayout() {
    return (
        <div className="antialiased noise-overlay min-h-screen flex flex-col bg-[#050507] grid-bg">
            <Outlet />
        </div>
    );
}
