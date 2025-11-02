package syncer

import "github.com/getcihub/cihub/core"

func diff(a, b *core.Installation) bool {
	switch {
	case a.Membership == nil || b.Membership == nil:
		return true
	case a.Membership.Role != b.Membership.Role:
		return true
	case a.Membership.State != b.Membership.State:
		return true
	default:
		return false
	}
}
