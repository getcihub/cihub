import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuGroup,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from '@/components/DropdownMenu';
import { useAuth } from '@/hooks/useAuth';
import { cx, focusRing } from '@/lib/utils';
import {
    RiArrowRightUpLine,
    RiLogoutBoxLine,
    RiSettingsLine,
} from '@remixicon/react';
import { useNavigate } from '@tanstack/react-router';
import React from 'react';

function DropdownUserProfile() {
    const [mounted, setMounted] = React.useState(false);
    const { user, logout } = useAuth();
    const navigate = useNavigate();

    React.useEffect(() => {
        setMounted(true);
    }, []);

    // Get user initials from login
    const getInitials = (login: string) => {
        return login
            .split(/[._-]/)
            .map((part) => part[0])
            .join('')
            .toUpperCase()
            .slice(0, 2);
    };

    if (!mounted || !user) {
        return null;
    }

    const initials = getInitials(user.login);

    return (
        <>
            <DropdownMenu>
                <DropdownMenuTrigger asChild>
                    <button
                        aria-label="open settings"
                        className={cx(
                            focusRing,
                            'group rounded-full p-1 transition-colors hover:bg-white/5 data-[state=open]:bg-white/10',
                        )}
                    >
                        {user.avatar_url ? (
                            <img
                                src={user.avatar_url}
                                alt={user.login}
                                className="size-8 shrink-0 rounded-full border border-white/10 object-cover"
                            />
                        ) : (
                            <span
                                className="flex size-8 shrink-0 items-center justify-center rounded-full border border-white/10 bg-white/5 font-mono text-xs text-white/70"
                                aria-hidden="true"
                            >
                                {initials}
                            </span>
                        )}
                    </button>
                </DropdownMenuTrigger>
                <DropdownMenuContent
                    align="end"
                    className="!min-w-[calc(var(--radix-dropdown-menu-trigger-width))]"
                >
                    <DropdownMenuLabel>
                        {user.email || user.login}
                    </DropdownMenuLabel>
                    <DropdownMenuSeparator />
                    <DropdownMenuGroup>
                        <DropdownMenuItem
                            onClick={() => navigate({ to: '/account' })}
                        >
                            <RiSettingsLine
                                className="mb-1 mr-2 size-4 shrink-0"
                                aria-hidden="true"
                            />
                            Account Settings
                        </DropdownMenuItem>
                    </DropdownMenuGroup>
                    <DropdownMenuSeparator />
                    <DropdownMenuGroup>
                        <DropdownMenuItem>
                            Changelog
                            <RiArrowRightUpLine
                                className="mb-1 ml-1 size-3 shrink-0 text-white/50"
                                aria-hidden="true"
                            />
                        </DropdownMenuItem>
                        <DropdownMenuItem
                            onClick={() =>
                                window.open('https://docs.cihub.io', '_blank')
                            }
                        >
                            Documentation
                            <RiArrowRightUpLine
                                className="mb-1 ml-1 size-3 shrink-0 text-white/50"
                                aria-hidden="true"
                            />
                        </DropdownMenuItem>
                        <DropdownMenuItem>
                            Join Slack community
                            <RiArrowRightUpLine
                                className="mb-1 ml-1 size-3 shrink-0 text-white/50"
                                aria-hidden="true"
                            />
                        </DropdownMenuItem>
                    </DropdownMenuGroup>
                    <DropdownMenuSeparator />
                    <DropdownMenuGroup>
                        <DropdownMenuItem onClick={logout}>
                            <RiLogoutBoxLine
                                className="mb-1 mr-2 size-4 shrink-0"
                                aria-hidden="true"
                            />
                            Sign out
                        </DropdownMenuItem>
                    </DropdownMenuGroup>
                </DropdownMenuContent>
            </DropdownMenu>
        </>
    );
}

export { DropdownUserProfile };
