package core

import "context"

const (
	MembershipRoleAdmin  = "admin"
	MembershipRoleMember = "member"
	MembershipRoleOwner  = "owner"
)

type (
	// Membership represents an individual membership
	// between an installation and a user.
	Membership struct {
		InstallationID int64  `json:"-"`
		UserID         int64  `json:"-"`
		Role           string `json:"role"`
		State          string `json:"state"`
		Created        int64  `json:"-"`
		Synced         int64  `json:"-"`
		Updated        int64  `json:"-"`
	}

	MembershipStore interface {
		// Find returns an org membership from the datastore.
		Find(ctx context.Context, installID, userID int64) (*Membership, error)

		// Update persists an updated org member to the datastore.
		Update(ctx context.Context, membership *Membership) error
	}
)
