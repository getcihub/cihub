package resource

import (
	"runtime"

	"github.com/getcihub/cihub/core"
)

// detectCPUArch detects the architecture of a CPU
func detectCPUArch() core.Arch {
	switch runtime.GOARCH {
	case "amd64":
		return core.ArchAmd64
	case "arm64":
		return core.ArchArm64
	default:
		return core.ArchUnknown
	}
}
