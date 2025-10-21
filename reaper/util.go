package reaper

import "time"

// helper function returns the current time.
var now = time.Now

// isExceeded is an helper function returning true
// if the time exceed the timeout duration.
func isExceeded(unix int64, reclaim time.Duration) bool {
	return now().After(time.Unix(unix, 0).Add(reclaim))
}
