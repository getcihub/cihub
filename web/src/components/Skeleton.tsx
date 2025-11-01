import { cx } from '../lib/utils'

interface SkeletonProps extends React.HTMLAttributes<HTMLDivElement> {
    className?: string
}

export function Skeleton({ className, ...props }: SkeletonProps) {
    return (
        <div
            className={cx(
                'animate-pulse rounded-md bg-gray-200',
                className
            )}
            {...props}
        />
    )
}
