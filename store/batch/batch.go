package batch

import (
	"context"

	"github.com/sirupsen/logrus"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
)

type store struct {
	db *db.DB
}

// New returns a new Batcher.
func New(db *db.DB) core.Batcher {
	return &store{db}
}

// Batch applies batch operations to synchronize the local installation
// and membership data with the remote GitHub state.
func (s *store) Batch(ctx context.Context, user *core.User, batch *core.Batch) error {
	return s.db.Update(func(execer db.Execer, binder db.Binder) error {
		// Process inserts: new installations and their memberships
		for _, installation := range batch.Insert {
			err := s.insertInstallation(execer, binder, installation)
			if err != nil {
				return err
			}

			err = s.insertMembership(execer, binder, user, installation)
			if err != nil {
				return err
			}
		}

		// Process updates: existing installations with changed membership data
		for _, installation := range batch.Update {
			err := s.updateMembership(execer, binder, user, installation)
			if err != nil {
				return err
			}
		}

		// Process revokes: remove memberships for installations no longer in GitHub
		for _, installation := range batch.Revoke {
			err := s.revokeMembership(execer, binder, user, installation)
			if err != nil {
				return err
			}
		}

		return nil
	})
}

func (s *store) insertInstallation(execer db.Execer, binder db.Binder, installation *core.Installation) error {
	params := map[string]interface{}{
		"installation_id":        installation.ID,
		"installation_login":     installation.Login,
		"installation_avatar":    installation.Avatar,
		"installation_type":      installation.Type,
		"installation_created":   installation.Created,
		"installation_suspended": installation.Suspended,
		"installation_updated":   installation.Updated,
	}

	// Select appropriate SQL based on database driver
	var sqlStmt string
	switch s.db.Driver() {
	case db.Postgres:
		sqlStmt = stmtInsertInstallationPG
	default:
		sqlStmt = stmtInsertInstallation
	}

	query, args, err := binder.BindNamed(sqlStmt, params)
	if err != nil {
		return err
	}

	_, err = execer.Exec(query, args...)
	return err
}

func (s *store) insertMembership(execer db.Execer, binder db.Binder, user *core.User, installation *core.Installation) error {
	if installation.Membership == nil {
		logrus.WithFields(
			logrus.Fields{
				"installation_id":    installation.ID,
				"installation_login": installation.Login,
				"user_login":         user.Login,
			},
		).Warnln("batch: installation missing membership")
		return nil
	}

	params := map[string]interface{}{
		"membership_installation_id": installation.ID,
		"membership_user_id":         user.ID,
		"membership_role":            installation.Membership.Role,
		"membership_state":           installation.Membership.State,
		"membership_created":         installation.Membership.Created,
		"membership_synced":          installation.Membership.Synced,
		"membership_updated":         installation.Membership.Updated,
	}

	stmt, args, err := binder.BindNamed(stmtInsertMembership, params)
	if err != nil {
		return err
	}

	_, err = execer.Exec(stmt, args...)
	return err
}

func (s *store) updateMembership(execer db.Execer, binder db.Binder, user *core.User, installation *core.Installation) error {
	if installation.Membership == nil {
		logrus.WithFields(
			logrus.Fields{
				"user_login":       user.Login,
				"installation_id":  installation.ID,
				"installation_log": installation.Login,
			},
		).Warnln("batch: installation missing membership")
		return nil
	}

	params := map[string]interface{}{
		"membership_installation_id": installation.ID,
		"membership_user_id":         user.ID,
		"membership_role":            installation.Membership.Role,
		"membership_state":           installation.Membership.State,
		"membership_created":         installation.Membership.Created,
		"membership_synced":          installation.Membership.Synced,
		"membership_updated":         installation.Membership.Updated,
	}

	stmt, args, err := binder.BindNamed(stmtUpdateMembership, params)
	if err != nil {
		return err
	}

	_, err = execer.Exec(stmt, args...)
	return err
}

func (s *store) revokeMembership(execer db.Execer, binder db.Binder, user *core.User, installation *core.Installation) error {
	// Delete both installation and membership records when revoking
	params := map[string]interface{}{
		"membership_user_id":         user.ID,
		"membership_installation_id": installation.ID,
	}

	stmt, args, err := binder.BindNamed(stmtDeleteMembership, params)
	if err != nil {
		return err
	}
	_, err = execer.Exec(stmt, args...)
	if err != nil {
		return err
	}

	// Delete the installation if no other users have memberships for it
	params = map[string]interface{}{"installation_id": installation.ID}
	stmt, args, err = binder.BindNamed(stmtDeleteInstallation, params)
	if err != nil {
		return err
	}

	_, err = execer.Exec(stmt, args...)
	return err
}

// SQLite syntax for idempotent insert
const stmtInsertInstallation = `
INSERT OR IGNORE INTO installations (
	installation_id,
	installation_login,
	installation_avatar,
	installation_type,
	installation_created,
	installation_suspended,
	installation_updated
) VALUES (
	:installation_id,
	:installation_login,
	:installation_avatar,
	:installation_type,
	:installation_created,
	:installation_suspended,
	:installation_updated
)
`

// PostgreSQL syntax for idempotent insert
const stmtInsertInstallationPG = `
INSERT INTO installations (
	installation_id,
	installation_login,
	installation_avatar,
	installation_type,
	installation_created,
	installation_suspended,
	installation_updated
) VALUES (
	:installation_id,
	:installation_login,
	:installation_avatar,
	:installation_type,
	:installation_created,
	:installation_suspended,
	:installation_updated
)
ON CONFLICT (installation_id) DO NOTHING
`

const stmtInsertMembership = `
INSERT INTO memberships (
	membership_installation_id,
	membership_user_id,
	membership_role,
	membership_state,
	membership_created,
	membership_synced,
	membership_updated
) VALUES (
	:membership_installation_id,
	:membership_user_id,
	:membership_role,
	:membership_state,
	:membership_created,
	:membership_synced,
	:membership_updated
)
`

const stmtUpdateMembership = `
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

const stmtDeleteMembership = `
DELETE FROM memberships
WHERE membership_user_id = :membership_user_id
  AND membership_installation_id = :membership_installation_id
`

const stmtDeleteInstallation = `
DELETE FROM installations
WHERE installation_id = :installation_id
  AND installation_id NOT IN (
    SELECT membership_installation_id FROM memberships
  )
`
