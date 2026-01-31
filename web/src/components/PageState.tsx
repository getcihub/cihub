import { motion } from 'framer-motion';
import { type ReactNode } from 'react';
import { type RemixiconComponentType } from '@remixicon/react';

interface EmptyStateProps {
    icon: RemixiconComponentType;
    title: string;
    description: string;
    action?: ReactNode;
}

export function EmptyState({ icon: Icon, title, description, action }: EmptyStateProps) {
    return (
        <motion.div
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5 }}
            className="flex flex-col items-center justify-center py-16"
        >
            <div className="mb-4 rounded-lg bg-white/5 p-3">
                <Icon className="size-6 text-white/40" />
            </div>
            <h3 className="mb-2 font-display text-lg text-white">{title}</h3>
            <p className="mb-6 max-w-md text-center font-mono text-xs text-muted">{description}</p>
            {action}
        </motion.div>
    );
}

export function LoadingState() {
    return (
        <div className="flex items-center justify-center py-16">
            <div className="font-mono text-sm text-muted">Loading...</div>
        </div>
    );
}
