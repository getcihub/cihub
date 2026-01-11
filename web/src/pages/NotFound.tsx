import { Link } from '@tanstack/react-router';
import { motion } from 'framer-motion';

export function NotFoundPage() {
    return (
        <div className="grid-bg flex min-h-screen items-center justify-center bg-[#050507]">
            <motion.div
                className="px-4 py-8 text-center"
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.5 }}
            >
                <div className="mb-8">
                    <h1 className="mb-2 font-display text-6xl font-bold text-white">
                        404
                    </h1>
                    <p className="font-mono text-lg text-white/50">
                        Page not found
                    </p>
                </div>

                <p className="mx-auto mb-8 max-w-md font-mono text-sm text-white/40">
                    The page you're looking for doesn't exist or has been moved.
                    Let's get you back on track.
                </p>

                <div className="flex flex-col justify-center gap-4 sm:flex-row">
                    <Link
                        to="/"
                        className="inline-flex items-center justify-center rounded-md bg-amber-500 px-6 py-2.5 font-mono text-sm text-white transition-colors hover:bg-amber-400"
                    >
                        Go to Dashboard
                    </Link>
                </div>
            </motion.div>
        </div>
    );
}
