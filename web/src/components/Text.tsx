import { cx } from '../lib/utils'

interface TextProps extends React.HTMLAttributes<HTMLDivElement> {
    variant?: 'body' | 'small' | 'xs'
    className?: string
}

const textVariants = {
    body: 'text-tremor-default text-tremor-content',
    small: 'text-tremor-default text-sm text-tremor-content',
    xs: 'text-gray-500 sm:text-sm/6',
}

export function Text({ variant = 'body', className, children, ...props }: TextProps) {
    return (
        <div className={cx(textVariants[variant], className)} {...props}>
            {children}
        </div>
    )
}
