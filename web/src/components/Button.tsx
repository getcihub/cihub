import { forwardRef, type ButtonHTMLAttributes, type ReactNode } from 'react';

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'danger' | 'ghost';
  size?: 'sm' | 'md' | 'lg';
  children: ReactNode;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
  function Button({ variant = 'primary', size = 'md', className = '', children, disabled, ...props }, ref) {
    const baseClasses = 'inline-flex items-center justify-center gap-2 rounded-md font-mono transition-all disabled:opacity-50 disabled:cursor-not-allowed';

    const variants = {
      primary: 'bg-amber-500 text-black hover:bg-amber-400 border border-amber-500',
      secondary: 'bg-white/[0.02] text-white hover:bg-white/5 border border-white/10 hover:border-white/20',
      danger: 'bg-red-500/10 text-red-400 hover:bg-red-500/20 border border-red-500/20 hover:border-red-500/30',
      ghost: 'text-white hover:bg-white/5',
    };

    const sizes = {
      sm: 'px-3 py-1.5 text-xs',
      md: 'px-4 py-2 text-sm',
      lg: 'px-5 py-3 text-base',
    };

    return (
      <button
        ref={ref}
        className={`${baseClasses} ${variants[variant]} ${sizes[size]} ${className}`}
        disabled={disabled}
        {...props}
      >
        {children}
      </button>
    );
  }
);
