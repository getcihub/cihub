package runner

import (
	"strings"

	"github.com/getcihub/cihub/core"
)

func convertRunnerStatus(s string) core.RunnerStatus {
	switch {
	case strings.EqualFold(s, "offline"):
		return core.RunnerStatusRegistered
	case strings.EqualFold(s, "online"), strings.EqualFold(s, "idle"):
		return core.RunnerStatusIdle
	default:
		return core.RunnerStatusUnknown
	}
}
