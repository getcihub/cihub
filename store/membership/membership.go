package membership

import (
	"context"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
)

type store struct {
	db *db.DB
}

// New returns a new MembershipStore.
func New(db *db.DB) core.MembershipStore {
	return &store{db}
}

// Find returns the membership for an installation and user.
func (s *store) Find(ctx context.Context, installID, userID int64) (*core.Membership, error) {
	out := &core.Membership{InstallationID: installID, UserID: userID}
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{
			"membership_installation_id": installID,
			"membership_user_id":         userID,
		}
		query, args, err := binder.BindNamed(queryFind, params)
		if err != nil {
			return err
		}
		row := queryer.QueryRow(query, args...)
		return scanRow(row, out)
	})
	return out, err
}

// Update persists an updated membership to the datastore.
func (s *store) Update(ctx context.Context, membership *core.Membership) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params := toParams(membership)
		stmt, args, err := binder.BindNamed(stmtUpdate, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

const queryBase = `
SELECT
	membership_installation_id,
	membership_user_id,
	membership_role,
	membership_state,
	membership_created,
	membership_synced,
	membership_updated
`

const queryFind = queryBase + `
FROM memberships
WHERE membership_installation_id = :membership_installation_id
  AND membership_user_id = :membership_user_id
`

const stmtUpdate = `
UPDATE memberships
SET
	membership_role    = :membership_role,
	membership_state   = :membership_state,
	membership_created = :membership_created,
	membership_synced  = :membership_synced,
	membership_updated = :membership_updated
WHERE membership_installation_id = :membership_installation_id
  AND membership_user_id = :membership_user_id
`
