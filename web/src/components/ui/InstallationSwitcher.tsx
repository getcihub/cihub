import {
    DropdownMenu,
    DropdownMenuContent,
    DropdownMenuGroup,
    DropdownMenuItem,
    DropdownMenuLabel,
    DropdownMenuSeparator,
    DropdownMenuTrigger,
} from '@/components/DropdownMenu';
import { useInstallation } from '@/hooks/useInstallation';
import { useInstallations } from '@/hooks/useInstallations';
import { useVarz } from '@/hooks/useVarz';
import { cx, focusRing } from '@/lib/utils';
import { RiAddLine, RiArrowDownSLine, RiCheckLine } from '@remixicon/react';
import { useNavigate } from '@tanstack/react-router';

export function InstallationSwitcher() {
    const navigate = useNavigate();
    const { selectedInstallation, selectInstallation } = useInstallation();
    const { data: installations = [] } = useInstallations();
    const { data: varz } = useVarz();

    if (!selectedInstallation) {
        return null;
    }

    const handleSelectInstallation = async (login: string) => {
        const installation = installations.find((i) => i.login === login);
        if (!installation) return;

        try {
            await selectInstallation(installation);
            navigate({ to: '/$login/machines', params: { login } });
        } catch (err) {
            console.error('Failed to select installation:', err);
        }
    };

    const handleAddInstallation = () => {
        if (varz?.github?.name) {
            // Redirect to GitHub App installation page
            window.location.href = `https://github.com/apps/${varz.github.name}/installations/new`;
        } else {
            // Fallback to installations page if app name is not available
            navigate({ to: '/' });
        }
    };

    return (
        <DropdownMenu>
            <DropdownMenuTrigger asChild>
                <button
                    className={cx(
                        focusRing,
                        'group rounded-lg p-2 hover:bg-gray-100 data-[state=open]:bg-gray-100 flex items-center gap-2',
                    )}
                    aria-label="Switch installation"
                >
                    <div className="flex items-center gap-2 min-w-0">
                        {selectedInstallation.avatar_url ? (
                            <img
                                src={selectedInstallation.avatar_url}
                                alt={selectedInstallation.login}
                                className="size-6 rounded-md border border-gray-200 object-cover flex-shrink-0"
                            />
                        ) : (
                            <div className="size-6 rounded-md bg-gradient-to-br from-blue-400 to-blue-600 flex items-center justify-center flex-shrink-0 text-white font-semibold text-xs">
                                {selectedInstallation.login
                                    .charAt(0)
                                    .toUpperCase()}
                            </div>
                        )}
                        <span className="text-sm font-medium text-gray-900 truncate">
                            {selectedInstallation.login}
                        </span>
                    </div>
                    <RiArrowDownSLine
                        className="size-4 text-gray-500 flex-shrink-0"
                        aria-hidden="true"
                    />
                </button>
            </DropdownMenuTrigger>
            <DropdownMenuContent align="start" className="w-56">
                <DropdownMenuLabel>Your Installations</DropdownMenuLabel>
                <DropdownMenuSeparator />
                <DropdownMenuGroup>
                    {installations.length > 0 ? (
                        installations.map((installation) => (
                            <DropdownMenuItem
                                key={installation.id}
                                onSelect={() =>
                                    handleSelectInstallation(installation.login)
                                }
                                className="flex items-center gap-3 cursor-pointer"
                            >
                                {installation.avatar_url ? (
                                    <img
                                        src={installation.avatar_url}
                                        alt={installation.login}
                                        className="size-5 rounded border border-gray-200 object-cover flex-shrink-0"
                                    />
                                ) : (
                                    <div className="size-5 rounded bg-gradient-to-br from-blue-400 to-blue-600 flex items-center justify-center flex-shrink-0 text-white font-semibold text-xs">
                                        {installation.login
                                            .charAt(0)
                                            .toUpperCase()}
                                    </div>
                                )}
                                <span className="flex-1 truncate">
                                    {installation.login}
                                </span>
                                {selectedInstallation.id ===
                                    installation.id && (
                                    <RiCheckLine
                                        className="size-4 text-green-600 flex-shrink-0"
                                        aria-hidden="true"
                                    />
                                )}
                            </DropdownMenuItem>
                        ))
                    ) : (
                        <div className="px-3 py-2 text-sm text-gray-500">
                            No installations available
                        </div>
                    )}
                </DropdownMenuGroup>
                <DropdownMenuSeparator />
                <DropdownMenuGroup>
                    <DropdownMenuItem
                        onSelect={handleAddInstallation}
                        className="flex items-center gap-2 cursor-pointer"
                    >
                        <RiAddLine
                            className="size-4 flex-shrink-0"
                            aria-hidden="true"
                        />
                        <span>Add another installation</span>
                    </DropdownMenuItem>
                </DropdownMenuGroup>
            </DropdownMenuContent>
        </DropdownMenu>
    );
}
