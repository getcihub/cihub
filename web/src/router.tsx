import { Router, createRootRoute, createRoute } from "@tanstack/react-router";

import App from "@/App";
import LoginPage from "@/pages/Login";
import InstallationsPage from "@/pages/Installation";
import MachinePage from "./pages/Machines";
import RunnersPage from "./pages/Runners";
import SettingsPage from "./pages/Settings";

// Root layout applied to all pages
const root = createRootRoute({
    component: App,
    errorComponent: null,
});

// Installations index route
const installationsRoute = createRoute({
    getParentRoute: () => root,
    path: '/',
    component: InstallationsPage
});

// Installation layout route with :owner param
const installationRoute = createRoute({
    getParentRoute: () => root,
    path: '/installations/$owner',
});

// Machines route nested under installation
const machinesRoute = createRoute({
    getParentRoute: () => installationRoute,
    path: '/machines',
    component: MachinePage,
});

// Runners route nested under installation
const runnersRoute = createRoute({
    getParentRoute: () => installationRoute,
    path: '/runners',
    component: RunnersPage,
});

// Login route
const loginRoute = createRoute({
    getParentRoute: () => root,
    path: "/login",
    component: LoginPage,
})

const settingsRoute = createRoute({
    getParentRoute: () => root,
    path: "/settings",
    component: SettingsPage,
})

// Create the route tree
const routeTree = root.addChildren([
    installationsRoute,
    loginRoute,
    settingsRoute,
    installationRoute.addChildren([
        machinesRoute,
        runnersRoute,
    ]),
])

// Create the router
export const router = new Router({ routeTree });

// Register router for type safety
declare module '@tanstack/react-router' {
    interface Register {
        router: typeof router;
    }
}
