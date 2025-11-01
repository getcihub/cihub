import { Link } from '@tanstack/react-router'

export function NotFoundPage() {
    return (
        <div className="flex items-center justify-center min-h-screen">
            <div className="text-center px-4 py-8">
                <div className="mb-8">
                    <h1 className="text-6xl font-bold text-gray-900 mb-2">
                        404
                    </h1>
                    <p className="text-xl text-gray-600">
                        Page not found
                    </p>
                </div>

                <p className="text-gray-500 mb-8 max-w-md mx-auto">
                    The page you're looking for doesn't exist or has been moved.
                    Let's get you back on track.
                </p>

                <div className="flex flex-col sm:flex-row gap-4 justify-center">
                    <Link
                        to="/"
                        className="inline-flex items-center justify-center px-6 py-2.5 text-sm font-medium text-white bg-blue-600 hover:bg-blue-700 rounded-lg transition-colors"
                    >
                        Go to Dashboard
                    </Link>
                </div>
            </div>
        </div>
    )
}
