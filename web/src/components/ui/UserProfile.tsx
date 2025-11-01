import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuGroup,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from "../DropdownMenu"
import { cx, focusRing } from "../../lib/utils"
import {
    RiArrowRightUpLine,
    RiLogoutBoxLine,
} from "@remixicon/react"
import { useAuth } from "../../hooks/useAuth"
import React from "react"

function DropdownUserProfile() {
    const [mounted, setMounted] = React.useState(false)
    const { user, logout } = useAuth()

    React.useEffect(() => {
        setMounted(true)
    }, [])

    // Get user initials from login
    const getInitials = (login: string) => {
        return login
            .split(/[._-]/)
            .map(part => part[0])
            .join('')
            .toUpperCase()
            .slice(0, 2)
    }

    if (!mounted || !user) {
        return null
    }

    const initials = getInitials(user.login)

    return (
        <>
            <DropdownMenu>
                <DropdownMenuTrigger asChild>
                    <button
                        aria-label="open settings"
                        className={cx(
                            focusRing,
                            "group rounded-full p-1 hover:bg-gray-100 data-[state=open]:bg-gray-100",
                        )}
                    >
                        {user.avatar ? (
                            <img
                                src={user.avatar}
                                alt={user.login}
                                className="size-8 shrink-0 rounded-full border border-gray-300 object-cover"
                            />
                        ) : (
                            <span
                                className="flex size-8 shrink-0 items-center justify-center rounded-full border border-gray-300 bg-white text-xs font-medium text-gray-700"
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
                        <DropdownMenuItem>
                            Changelog
                            <RiArrowRightUpLine
                                className="mb-1 ml-1 size-3 shrink-0 text-gray-500"
                                aria-hidden="true"
                            />
                        </DropdownMenuItem>
                        <DropdownMenuItem>
                            Documentation
                            <RiArrowRightUpLine
                                className="mb-1 ml-1 size-3 shrink-0 text-gray-500"
                                aria-hidden="true"
                            />
                        </DropdownMenuItem>
                        <DropdownMenuItem>
                            Join Slack community
                            <RiArrowRightUpLine
                                className="mb-1 ml-1 size-3 shrink-0 text-gray-500"
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
    )
}

export { DropdownUserProfile }
