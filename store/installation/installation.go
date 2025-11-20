package installation

import (
	"context"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
)

type store struct {
	db *db.DB
}

// New returns a new InstallationStore.
func New(db *db.DB) core.InstallationStore {
	return &store{db}
}

// Count returns a count of active installations from the datastore.
func (s *store) Count(ctx context.Context) (int64, error) {
	var out int64
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		return queryer.QueryRow(queryCount).Scan(&out)
	})
	return out, err
}

// Create persists a new installation to the datastore.
func (s *store) Create(ctx context.Context, installation *core.Installation) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params := ToParams(installation)
		stmt, args, err := binder.BindNamed(stmtInsert, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

// Delete deletes an installation from the datastore.
func (s *store) Delete(ctx context.Context, installation *core.Installation) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params := map[string]any{"installation_id": installation.ID}
		stmt, args, err := binder.BindNamed(stmtDelete, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

// Find returns installation by ID.
func (s *store) Find(ctx context.Context, id int64) (*core.Installation, error) {
	out := &core.Installation{ID: id}
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{"installation_id": id}
		query, args, err := binder.BindNamed(queryFind, params)
		if err != nil {
			return err
		}
		row := queryer.QueryRow(query, args...)
		return scanRow(row, out)
	})
	return out, err
}

// FindLogin returns installation by login
func (s *store) FindLogin(ctx context.Context, login string) (*core.Installation, error) {
	out := &core.Installation{Login: login}
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{"installation_login": login}
		query, args, err := binder.BindNamed(queryFindLogin, params)
		if err != nil {
			return err
		}
		row := queryer.QueryRow(query, args...)
		return scanRow(row, out)
	})
	return out, err
}

// List returns a slice of installations for a user from the datastore.
func (s *store) List(ctx context.Context, user *core.User) ([]*core.Installation, error) {
	var out []*core.Installation
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]any{"user_id": user.ID}
		stmt, args, err := binder.BindNamed(queryList, params)
		if err != nil {
			return err
		}

		rows, err := queryer.Query(stmt, args...)
		if err != nil {
			return err
		}
		out, err = scanRows(rows)
		return err
	})
	return out, err
}

// Update persists an updated installation to the datastore.
func (s *store) Update(ctx context.Context, installation *core.Installation) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params := ToParams(installation)
		stmt, args, err := binder.BindNamed(stmtUpdate, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

const queryCount = `
SELECT COUNT(*)
FROM installations
WHERE installation_suspended = 0
`

const queryBase = `
SELECT
	installation_id,
	installation_login,
	installation_avatar,
	installation_type,
	installation_created,
	installation_suspended,
	installation_updated
`

const queryFind = queryBase + `
FROM installations
WHERE installation_id = :installation_id
`

const queryFindLogin = queryBase + `
FROM installations
WHERE installation_login = :installation_login
`

const queryList = queryBase + `
FROM installations
INNER JOIN memberships ON installations.installation_id = memberships.membership_installation_id
WHERE memberships.membership_user_id = :user_id
ORDER BY installation_login
`

const stmtDelete = `
DELETE FROM installations WHERE installation_id = :installation_id
`

const stmtInsert = `
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
`

const stmtUpdate = `
UPDATE installations
SET
	installation_login     = :installation_login,
	installation_avatar    = :installation_avatar,
	installation_type      = :installation_type,
	installation_created   = :installation_created,
	installation_suspended = :installation_suspended,
	installation_updated   = :installation_updated
WHERE installation_id = :installation_id
`
