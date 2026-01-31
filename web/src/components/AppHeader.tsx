import { useState } from 'react';
import { useNavigate, useRouterState, useParams } from '@tanstack/react-router';
import { useQuery } from '@tanstack/react-query';
import * as DropdownMenu from '@radix-ui/react-dropdown-menu';
import { RiArrowDownSLine, RiBookOpenLine, RiCheckLine, RiLogoutBoxLine, RiUserLine } from '@remixicon/react';
import type { APIResponse, Installation, User } from '@/types/api';

export function AppHeader() {
    const navigate = useNavigate();
    const router = useRouterState();
    const { owner } = useParams({ strict: false }) as { owner?: string };
    const currentPath = router.location.pathname;
    const [dropdownOpen, setDropdownOpen] = useState(false);

    // Fetch all installations
    const { data: installationsResponse } = useQuery<APIResponse<Installation[]>>({
        queryKey: ['installations'],
        queryFn: async () => {
            const res = await fetch('/api/user/installations');
            if (!res.ok) throw new Error('Failed to fetch installations');
            return res.json();
        },
    });
    const { data: userResponse } = useQuery<APIResponse<User>>({
        queryKey: ['user'],
        queryFn: async () => {
            const res = await fetch('/api/user');
            if (!res.ok) throw new Error('Failed to fetch user');
            return res.json();
        },
    });

    const installations = installationsResponse?.data || [];
    const currentInstallation = installations.find((i) => i.name === owner);
    const currentUser = userResponse?.data;
    const ownerLabel = owner || 'Select installation';

    const tabs = [
        { name: 'Machines', path: 'machines', disabled: false },
        { name: 'Runners', path: 'runners', disabled: false },
    ];

    const handleInstallationChange = (newOwner: string) => {
        // Determine which tab to navigate to based on current path
        const currentTab = tabs.find(tab => currentPath.includes(tab.path));
        const targetPath = currentTab?.path || 'machines';

        navigate({
            to: `/installations/$owner/${targetPath}`,
            params: { owner: newOwner }
        });
        setDropdownOpen(false);
    };

    // Check if current path matches a tab
    const getActiveTab = () => {
        return tabs.find(tab => currentPath.includes(tab.path));
    };

    return (
        <div className="sticky top-0 z-50 border-b border-white/5 bg-[#050507]/95 backdrop-blur">
            <div className="mx-auto max-w-7xl px-4 sm:px-8">
                <div className="flex items-center justify-between py-4">
                    {/* Left: Installation Selector */}
                    <div className="flex items-center gap-4">
                        <DropdownMenu.Root open={dropdownOpen} onOpenChange={setDropdownOpen}>
                            <DropdownMenu.Trigger asChild>
                                <button className="flex items-center gap-3 group hover:bg-white/5 rounded-lg px-3 py-2 transition-colors">
                                    {currentInstallation?.avatar && (
                                        <img
                                            src={currentInstallation.avatar}
                                            alt={owner}
                                            className="size-8 rounded-lg border border-white/10 object-cover"
                                        />
                                    )}
                                    <div className="flex items-center gap-2">
                                        <span className="font-mono text-sm text-white">{ownerLabel}</span>
                                        <RiArrowDownSLine className="size-4 text-white/40 group-hover:text-white/60 transition-colors" />
                                    </div>
                                </button>
                            </DropdownMenu.Trigger>

                            <DropdownMenu.Portal>
                                <DropdownMenu.Content
                                    className="z-50 min-w-[280px] rounded-lg border border-white/10 bg-[#050507] p-2 shadow-2xl animate-fade-in"
                                    sideOffset={5}
                                >
                                    <div className="mb-2 px-2 py-1.5">
                                        <p className="font-mono text-xs uppercase tracking-wider text-secondary">
                                            Switch Installation
                                        </p>
                                    </div>
                                    {installations.map((installation) => {
                                        const isActive = installation.name === owner;
                                        return (
                                            <DropdownMenu.Item
                                                key={installation.id}
                                                className="flex items-center gap-3 rounded-md px-2 py-2 font-mono text-sm text-white outline-none cursor-pointer hover:bg-white/5 focus:bg-white/5"
                                                onSelect={() => handleInstallationChange(installation.name)}
                                            >
                                                <img
                                                    src={installation.avatar}
                                                    alt={installation.name}
                                                    className="size-6 rounded border border-white/10 object-cover"
                                                />
                                                <span className="flex-1">{installation.name}</span>
                                                {isActive && (
                                                    <RiCheckLine className="size-4 text-amber-400" />
                                                )}
                                            </DropdownMenu.Item>
                                        );
                                    })}

                                    <DropdownMenu.Separator className="my-2 h-px bg-white/5" />

                                    <DropdownMenu.Item
                                        className="flex items-center gap-2 rounded-md px-2 py-2 font-mono text-sm text-white/70 outline-none cursor-pointer hover:bg-white/5 focus:bg-white/5"
                                        onSelect={() => navigate({ to: '/' })}
                                    >
                                        View All Installations
                                    </DropdownMenu.Item>
                                </DropdownMenu.Content>
                            </DropdownMenu.Portal>
                        </DropdownMenu.Root>
                    </div>

                    {/* Right: Navigation Tabs */}
                    <div className="flex items-center gap-2">
                        {owner && (
                            <div className="hidden sm:flex items-center gap-1">
                                {tabs.map((tab) => {
                                    const activeTab = getActiveTab();
                                    const isActive = activeTab?.name === tab.name;
                                    const isDisabled = tab.disabled;

                                    if (isDisabled) {
                                        return (
                                            <div
                                                key={tab.name}
                                                className="relative px-4 py-2 cursor-not-allowed"
                                                title="Coming soon"
                                            >
                                                <span className="font-mono text-xs uppercase tracking-wider text-white/30">
                                                    {tab.name}
                                                </span>
                                            </div>
                                        );
                                    }

                                    return (
                                        <button
                                            key={tab.name}
                                            onClick={() => navigate({
                                                to: `/installations/$owner/${tab.path}`,
                                                params: { owner: owner || '' }
                                            })}
                                            className="relative px-4 py-2"
                                        >
                                            <span
                                                className={`font-mono text-xs uppercase tracking-wider transition-colors ${
                                                    isActive ? 'text-white' : 'text-white/50 hover:text-white/70'
                                                }`}
                                            >
                                                {tab.name}
                                            </span>
                                            {isActive && (
                                                <div className="absolute bottom-0 left-0 right-0 h-0.5 bg-amber-500" />
                                            )}
                                        </button>
                                    );
                                })}
                            </div>
                        )}
                        <DropdownMenu.Root>
                            <DropdownMenu.Trigger asChild>
                                <button className="flex items-center gap-2 rounded-lg border border-white/10 bg-white/5 px-3 py-2 font-mono text-xs text-white/70 transition-colors hover:bg-white/10 hover:text-white">
                                    {currentUser?.avatar_url ? (
                                        <img
                                            src={currentUser.avatar_url}
                                            alt={currentUser.login || ownerLabel}
                                            className="size-5 rounded-md border border-white/10 object-cover"
                                        />
                                    ) : (
                                        <RiUserLine className="size-4 text-white/60" />
                                    )}
                                    Account
                                    <RiArrowDownSLine className="size-4 text-white/40" />
                                </button>
                            </DropdownMenu.Trigger>
                            <DropdownMenu.Portal>
                                <DropdownMenu.Content
                                    className="z-50 min-w-[200px] rounded-lg border border-white/10 bg-[#050507] p-2 shadow-2xl animate-fade-in"
                                    sideOffset={6}
                                    align="end"
                                >
                                    <DropdownMenu.Item
                                        className="flex items-center gap-2 rounded-md px-2 py-2 font-mono text-xs text-white/70 outline-none cursor-pointer hover:bg-white/5 focus:bg-white/5"
                                        onSelect={() => navigate({ to: '/settings' })}
                                    >
                                        <RiUserLine className="size-4 text-white/40" />
                                        Your profile
                                    </DropdownMenu.Item>
                                    <DropdownMenu.Item
                                        className="flex items-center gap-2 rounded-md px-2 py-2 font-mono text-xs text-white/70 outline-none cursor-pointer hover:bg-white/5 focus:bg-white/5"
                                        onSelect={() => window.open('https://github.com/getcihub/cihub', '_blank')}
                                    >
                                        <RiBookOpenLine className="size-4 text-white/40" />
                                        Docs
                                    </DropdownMenu.Item>
                                    <DropdownMenu.Separator className="my-2 h-px bg-white/5" />
                                    <DropdownMenu.Item
                                        className="flex items-center gap-2 rounded-md px-2 py-2 font-mono text-xs text-red-400 outline-none cursor-pointer hover:bg-red-500/10 focus:bg-red-500/10"
                                        onSelect={() => { window.location.href = '/logout'; }}
                                    >
                                        <RiLogoutBoxLine className="size-4 text-red-400" />
                                        Sign Out
                                    </DropdownMenu.Item>
                                </DropdownMenu.Content>
                            </DropdownMenu.Portal>
                        </DropdownMenu.Root>
                    </div>
                </div>
            </div>
        </div>
    );
}
