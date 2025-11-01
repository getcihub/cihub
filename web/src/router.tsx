import { RootRoute, Route, Router } from '@tanstack/react-router'
import { LoginPage } from './pages/Login'
import { DashboardPage } from './pages/Dashboard'
import { MachinesPage } from './pages/Machines'
import { SettingsPage } from './pages/Settings'
import { NotFoundPage } from './pages/NotFound'
import App from './App'
import { RootLayout } from './layouts/RootLayout'
import { ConsoleLayout } from './layouts/ConsoleLayout'

// Root route layout - applies to all pages
const rootRoute = new RootRoute({
    component: App,
    notFoundComponent: NotFoundPage,
})

// Root layout - global styles for all pages
const rootLayoutRoute = new Route({
    getParentRoute: () => rootRoute,
    id: 'root',
    component: RootLayout,
})

// Console layout - shared template for authenticated console pages
const consoleLayoutRoute = new Route({
    getParentRoute: () => rootLayoutRoute,
    id: 'console',
    component: ConsoleLayout,
})

// Dashboard index route
const dashboardRoute = new Route({
    getParentRoute: () => consoleLayoutRoute,
    path: '/',
    component: DashboardPage,
})

// Agents route - uses same layout
const agentsRoute = new Route({
    getParentRoute: () => consoleLayoutRoute,
    path: '/machines',
    component: MachinesPage,
})

// Settings route - uses same layout
const settingsRoute = new Route({
    getParentRoute: () => consoleLayoutRoute,
    path: '/settings',
    component: SettingsPage,
})

// Login route - uses root layout only
const loginRoute = new Route({
    getParentRoute: () => rootLayoutRoute,
    path: '/login',
    component: LoginPage,
})

// Create the route tree
const routeTree = rootRoute.addChildren([
    rootLayoutRoute.addChildren([
        consoleLayoutRoute.addChildren([dashboardRoute, agentsRoute, settingsRoute]),
        loginRoute,
    ]),
])

// Create the router
export const router = new Router({ routeTree })

// Register router for type safety
declare module '@tanstack/react-router' {
    interface Register {
        router: typeof router
    }
}
