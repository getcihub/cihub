import { Divider } from "../components/Divider"

export function MachinesPage() {
    return (
        <main>
            <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
                <div>
                    <h1 className="text-2xl font-semibold text-gray-900">
                        Machines
                    </h1>
                    <p className="text-gray-500 sm:text-sm/6">
                        Machines registered within your organization
                    </p>
                </div>
            </div>
            <Divider />
            <section className="mt-8">
            </section>
        </main>
    )
}
