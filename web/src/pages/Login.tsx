import { motion } from "framer-motion";
import { RiGithubFill } from "@remixicon/react";
import { Suspense } from "react";
import { Logo } from "@/components/Logo";

function LoginContent() {

    return (
        <div className="grid-bg flex min-h-screen flex-col items-center justify-center bg-[#050507] px-4 lg:px-6">
            {/* Ambient glow effect */}
            <div
                className="pointer-events-none fixed left-1/2 top-1/2 h-[600px] w-[600px] -translate-x-1/2 -translate-y-1/2 opacity-[0.03]"
                style={{
                    background:
                        'radial-gradient(circle, #ffffff 0%, transparent 70%)',
                }}
            />

            <motion.div
                className="relative z-10 sm:mx-auto sm:w-full sm:max-w-sm"
                initial={{ opacity: 0, y: 20 }}
                animate={{ opacity: 1, y: 0 }}
                transition={{ duration: 0.5 }}
            >
                {/* Logo container */}
                <motion.div
                    className="relative mx-auto w-fit rounded-xl bg-white/[0.02] p-4 ring-1 ring-white/10"
                    initial={{ opacity: 0, scale: 0.9 }}
                    animate={{ opacity: 1, scale: 1 }}
                    transition={{ duration: 0.5, delay: 0.1 }}
                >
                    <div className="absolute left-[9%] top-[9%] size-1 rounded-full bg-white/10" />
                    <div className="absolute right-[9%] top-[9%] size-1 rounded-full bg-white/10" />
                    <div className="absolute bottom-[9%] left-[9%] size-1 rounded-full bg-white/10" />
                    <div className="absolute bottom-[9%] right-[9%] size-1 rounded-full bg-white/10" />
                    <div className="w-fit rounded-lg bg-amber-500 p-3 shadow-lg shadow-amber-500/20">
                        <Logo className="size-8" />
                    </div>
                </motion.div>

                {/* Title */}
                <motion.h2
                    className="mt-6 text-center font-display text-2xl text-white"
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.5, delay: 0.2 }}
                >
                    Sign in to CIHub
                </motion.h2>

                {/* Login button */}
                <motion.div
                    className="mt-8"
                    initial={{ opacity: 0, y: 10 }}
                    animate={{ opacity: 1, y: 0 }}
                    transition={{ duration: 0.5, delay: 0.3 }}
                >
                    <a
                        href="/auth/login"
                        className="inline-flex w-full items-center justify-center gap-2 rounded-md border border-white/10 bg-white/[0.02] px-4 py-3 font-mono text-sm text-white transition-all hover:bg-white/5 hover:border-white/20"
                    >
                        <RiGithubFill className="size-5" aria-hidden={true} />
                        Continue with GitHub
                    </a>
                </motion.div>

                {/* Helper text */}
                <motion.p
                    className="mt-6 text-center font-mono text-xs text-white/40"
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    transition={{ duration: 0.5, delay: 0.4 }}
                >
                    No account? We'll create one for you.
                </motion.p>
            </motion.div>
        </div>
    );
}

export default function LoginPage() {
    return (
        <Suspense
            fallback={
                <div className="flex-1 bg-[#050507] grid-bg flex items-center justify-center">
                    <div className="font-mono text-muted text-sm">Loading...</div>
                </div>
            }
        >
            <LoginContent />
        </Suspense>
    )
}
