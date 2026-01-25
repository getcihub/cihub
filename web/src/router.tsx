import App from '@/App';
import { AuthLayout } from '@/layouts/AuthLayout';
import { InstallationLayout } from '@/layouts/InstallationLayout';
import { RootLayout } from '@/layouts/RootLayout';
import { AccountPage } from '@/pages/Account';
import { InstallationsPage } from '@/pages/Installations';
import { LoginPage } from '@/pages/Login';
import { MachineDetailPage } from '@/pages/MachineDetail';
import { MachinesPage } from '@/pages/Machines';
import { NotFoundPage } from '@/pages/NotFound';
import { OverviewPage } from '@/pages/Overview';
import { RunnerDetailPage } from '@/pages/RunnerDetail';
import { RunnersPage } from '@/pages/Runners';
import { SettingsPage } from '@/pages/Settings';
import { RootRoute, Route, Router } from '@tanstack/react-router';

// Root route layout - applies to all pages
const rootRoute = new RootRoute({
    component: App,
    notFoundComponent: NotFoundPage,
});

// Root layout - global styles for all pages
const rootLayoutRoute = new Route({
    getParentRoute: () => rootRoute,
    id: 'root',
    component: RootLayout,
});

// Auth layout - for authenticated routes that don't require installation context
const authLayoutRoute = new Route({
    getParentRoute: () => rootLayoutRoute,
    id: 'auth',
    component: AuthLayout,
});

// Installations index route (protected, no installation context needed)
const installationsRoute = new Route({
    getParentRoute: () => authLayoutRoute,
    path: '/',
    component: InstallationsPage,
});

// Account route - protected, no installation context needed
const accountRoute = new Route({
    getParentRoute: () => authLayoutRoute,
    path: '/account',
    component: AccountPage,
});

// Installation layout - for routes that require installation context
// Scoped to a specific installation login parameter
const installationLayoutRoute = new Route({
    getParentRoute: () => rootRoute,
    path: '/$login',
    component: InstallationLayout,
});

// Machines route - requires installation context
const machinesRoute = new Route({
    getParentRoute: () => installationLayoutRoute,
    path: '/machines',
    component: MachinesPage,
});

// Machine detail route - requires installation context
const machineDetailRoute = new Route({
    getParentRoute: () => installationLayoutRoute,
    path: '/machines/$name',
    component: MachineDetailPage,
});

// Runners route - requires installation context
const runnersRoute = new Route({
    getParentRoute: () => installationLayoutRoute,
    path: '/runners',
    component: RunnersPage,
});

// Runner detail route - requires installation context
const runnerDetailRoute = new Route({
    getParentRoute: () => installationLayoutRoute,
    path: '/runners/$name',
    component: RunnerDetailPage,
});

// Overview route - requires installation context (index route)
const overviewRoute = new Route({
    getParentRoute: () => installationLayoutRoute,
    path: '/',
    component: OverviewPage,
});

// Settings route - requires installation context
const settingsRoute = new Route({
    getParentRoute: () => installationLayoutRoute,
    path: '/settings',
    component: SettingsPage,
});

// Login route - uses root layout only
const loginRoute = new Route({
    getParentRoute: () => rootLayoutRoute,
    path: '/login',
    component: LoginPage,
});

// Create the route tree
const routeTree = rootRoute.addChildren([
    rootLayoutRoute.addChildren([
        authLayoutRoute.addChildren([installationsRoute, accountRoute]),
        loginRoute,
    ]),
    installationLayoutRoute.addChildren([
        overviewRoute,
        machinesRoute,
        machineDetailRoute,
        runnersRoute,
        runnerDetailRoute,
        settingsRoute,
    ]),
]);

// Create the router
export const router = new Router({ routeTree });

// Register router for type safety
declare module '@tanstack/react-router' {
    interface Register {
        router: typeof router;
    }
}
