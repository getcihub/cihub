import { Divider } from "../components/Divider"

export function DashboardPage() {
    return (
        <main>
            <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
                <div>
                    <h1 className="text-2xl font-semibold text-gray-900">
                        Jobs queue
                    </h1>
                    <p className="text-gray-500 sm:text-sm/6">
                        Jobs queued or in progress within your organization
                    </p>
                </div>
            </div>
            <Divider />
            <section className="mt-8 space-y-6">
            </section>
        </main>
    )
}
