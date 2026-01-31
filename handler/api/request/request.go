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
