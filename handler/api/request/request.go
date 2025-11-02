package request

import (
	"context"

	"github.com/getcihub/cihub/core"
)

type key int

const (
	userKey key = iota
	installationKey
	membershipKey
)

// WithUser returns a copy of parent in which the user value is set
func WithUser(parent context.Context, user *core.User) context.Context {
	return context.WithValue(parent, userKey, user)
}

// UserFrom returns the value of the user key on the ctx
func UserFrom(ctx context.Context) (*core.User, bool) {
	user, ok := ctx.Value(userKey).(*core.User)
	return user, ok
}

// WithMemberWithMembership returns a copy of parent in which the membership value is set
func WithMembership(parent context.Context, membership *core.Membership) context.Context {
	return context.WithValue(parent, membershipKey, membership)
}

// MembershipFrom returns the value of the perm key on the ctx
func MembershipFrom(ctx context.Context) (*core.Membership, bool) {
	perm, ok := ctx.Value(membershipKey).(*core.Membership)
	return perm, ok
}

// WithInstallation returns a copy of parent in which the installation value is set
func WithInstallation(parent context.Context, installation *core.Installation) context.Context {
	return context.WithValue(parent, installationKey, installation)
}

// InstallationFrom returns the value of the installation key on the ctx
func InstallationFrom(ctx context.Context) (*core.Installation, bool) {
	repo, ok := ctx.Value(installationKey).(*core.Installation)
	return repo, ok
}
