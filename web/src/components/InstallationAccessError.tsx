import { RiErrorWarningLine, RiShieldLine } from '@remixicon/react';
import { useNavigate } from '@tanstack/react-router';

import { Button } from './Button';
import { Card } from './Card';

interface InstallationAccessErrorProps {
    status: number;
    reason?: string;
}

export function InstallationAccessError({
    status,
    reason,
}: InstallationAccessErrorProps) {
    const navigate = useNavigate();

    const is404 = status === 404;

    const title = is404 ? 'Installation Not Found' : 'Access Denied';
    const icon = is404 ? RiErrorWarningLine : RiShieldLine;
    const iconColor = is404 ? 'text-orange-600' : 'text-red-600';
    const bgColor = is404 ? 'bg-orange-50' : 'bg-red-50';
    const borderColor = is404 ? 'border-orange-200' : 'border-red-200';
    const textColor = is404 ? 'text-orange-800' : 'text-red-800';

    const description = is404
        ? "The installation you're looking for doesn't exist or has been deleted."
        : "You don't have permission to access this installation. You may need to be invited as a member.";

    const Icon = icon;

    return (
        <div className="flex items-center justify-center min-h-screen">
            <Card
                className={`${bgColor} ${borderColor} p-8 text-center max-w-2xl w-full mx-4`}
            >
                <Icon
                    className={`size-12 ${iconColor} mx-auto mb-4`}
                    aria-hidden="true"
                />
                <h1 className={`text-2xl font-bold ${textColor} mb-2`}>
                    {title}
                </h1>
                <p className={textColor}>{description}</p>
                {reason && (
                    <p className={`text-sm ${textColor} mt-3 opacity-75`}>
                        Error reason:{' '}
                        <span className="font-mono">{reason}</span>
                    </p>
                )}
                <div className="mt-6 flex gap-3 justify-center">
                    <Button
                        onClick={() => navigate({ to: '/' })}
                        className="gap-2"
                    >
                        View Installations
                    </Button>
                    <Button
                        onClick={() => window.history.back()}
                        variant="secondary"
                    >
                        Go Back
                    </Button>
                </div>
            </Card>
        </div>
    );
}
