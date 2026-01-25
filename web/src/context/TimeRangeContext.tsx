import {
    createContext,
    useContext,
    useState,
    useEffect,
    useCallback,
    useTransition,
} from 'react';
import type { ReactNode } from 'react';
import { useLocation, useNavigate } from '@tanstack/react-router'
import { DEFAULT_DAYS } from '@/lib/constants';
import {
    parseTimeRangeFromParams,
    getDateRange,
    getTimeRangeLabel,
} from '@/lib/dateUtils';
import type { TimeRange } from '@/lib/dateUtils';

interface TimeRangeContextValue {
    /** The current time range (relative or absolute) */
    range: TimeRange;
    /** Set the time range */
    setRange: (range: TimeRange) => void;
    /** Convenience: set relative days (backwards compat) */
    setDays: (days: number) => void;
    /** Convenience: get days value (for relative ranges) */
    days: number;
    /** Get date params for API calls */
    getDateParams: () => { startDate: string; endDate: string };
    /** Get display label */
    getDisplayLabel: () => string;
    /** Is a transition pending */
    isPending: boolean;
}

const TimeRangeContext = createContext<TimeRangeContextValue | null>(null);

export function TimeRangeProvider({ children }: { children: ReactNode }) {
    // TanStack Router equivalents:
    // - useSearch() gives you an object (typed if you configured validateSearch)
    // - useLocation() gives you pathname, searchStr, etc.
    // - useNavigate() navigates (replace by default via { replace: true })
    const location = useLocation()
    const navigate = useNavigate()

    // We still want URLSearchParams because your parser expects that.
    const searchParams = new URLSearchParams(location.searchStr)

    const [isPending, startTransition] = useTransition()

    // Parse initial range from URL
    const initialRange = parseTimeRangeFromParams(searchParams, DEFAULT_DAYS)
    const [range, setRangeState] = useState<TimeRange>(initialRange)

    // Sync state from URL changes (e.g., back/forward navigation)
    useEffect(() => {
        const urlRange = parseTimeRangeFromParams(
            new URLSearchParams(location.searchStr),
            DEFAULT_DAYS,
        )

        // Compare ranges to avoid unnecessary updates
        if (urlRange.type === 'relative' && range.type === 'relative') {
            if (urlRange.days !== range.days) setRangeState(urlRange)
        } else if (urlRange.type === 'absolute' && range.type === 'absolute') {
            if (
                urlRange.startDate !== range.startDate ||
                urlRange.endDate !== range.endDate
            ) {
                setRangeState(urlRange)
            }
        } else if (urlRange.type !== range.type) {
            setRangeState(urlRange)
        }
    }, [location.searchStr, range])

    // Update both state and URL when range changes
    const setRange = useCallback(
        (newRange: TimeRange) => {
            startTransition(() => {
                setRangeState(newRange)

                // Preserve other params
                const params = new URLSearchParams(location.searchStr)
                params.delete('days')
                params.delete('start')
                params.delete('end')

                // Add range params
                if (newRange.type === 'relative') {
                    params.set('days', newRange.days.toString())
                } else {
                    params.set('start', newRange.startDate)
                    params.set('end', newRange.endDate)
                }

                navigate({
                    to: location.pathname,
                    search: params.toString() ? `?${params.toString()}` : '',
                    replace: true,
                    resetScroll: false, // equivalent-ish to scroll: false
                })
            })
        },
        [location.pathname, location.searchStr, navigate],
    )

    const setDays = useCallback(
        (days: number) => setRange({ type: 'relative', days }),
        [setRange],
    )

    const days =
        range.type === 'relative'
            ? range.days
            : Math.ceil(
                (new Date(range.endDate).getTime() -
                    new Date(range.startDate).getTime()) /
                (1000 * 60 * 60 * 24),
            ) + 1

    const getDateParams = useCallback(() => getDateRange(range), [range])
    const getDisplayLabel = useCallback(() => getTimeRangeLabel(range), [range])

    return (
        <TimeRangeContext.Provider
            value={{
                range,
                setRange,
                setDays,
                days,
                getDateParams,
                getDisplayLabel,
                isPending,
            }}
        >
            {children}
        </TimeRangeContext.Provider>
    )
}

export function useTimeRange() {
    const context = useContext(TimeRangeContext);
    if (!context) {
        throw new Error('useTimeRange must be used within a TimeRangeProvider');
    }
    return context;
}
