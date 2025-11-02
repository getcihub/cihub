package user

import (
	"context"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/logger"
	"github.com/getcihub/cihub/store/shared/db"
	"github.com/getcihub/cihub/store/shared/encrypter"
)

type store struct {
	db  *db.DB
	enc encrypter.Encrypter
}

// New returns a new UserStore.
func New(db *db.DB, enc encrypter.Encrypter) core.UserStore {
	return &store{db, enc}
}

func (s *store) Count(context.Context) (int64, error) {
	var out int64
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		return queryer.QueryRow(queryCount).Scan(&out)
	})
	return out, err
}

func (s *store) Create(ctx context.Context, user *core.User) error {
	if s.db.Driver() == db.Postgres {
		return s.createPostgres(ctx, user)
	}
	return s.create(ctx, user)
}

func (s *store) create(ctx context.Context, user *core.User) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params, err := toParams(s.enc, user)
		if err != nil {
			return err
		}
		stmt, args, err := binder.BindNamed(stmtInsert, params)
		if err != nil {
			return err
		}
		res, err := execer.Exec(stmt, args...)
		if err != nil {
			return err
		}
		user.ID, err = res.LastInsertId()
		return err
	})
}

func (s *store) createPostgres(ctx context.Context, user *core.User) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params, err := toParams(s.enc, user)
		if err != nil {
			return err
		}
		stmt, args, err := binder.BindNamed(stmtInsertPg, params)
		if err != nil {
			return err
		}
		return execer.QueryRow(stmt, args...).Scan(&user.ID)
	})
}

// Delete deletes a user from the datastore.
func (s *store) Delete(ctx context.Context, user *core.User) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params := map[string]any{"user_id": user.ID}
		stmt, args, err := binder.BindNamed(stmtDelete, params)
		if err != nil {
			return err
		}
		_, err = execer.Exec(stmt, args...)
		return err
	})
}

// Find returns a user from the datastore by its ID.
func (s *store) Find(ctx context.Context, id int64) (*core.User, error) {
	out := &core.User{ID: id}
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{"user_id": id}
		query, args, err := binder.BindNamed(queryFind, params)
		if err != nil {
			return err
		}
		row := queryer.QueryRow(query, args...)
		return scanRow(s.enc, row, out)
	})
	return out, err
}

// FindLogin returns a user from the datastore by its login.
func (s *store) FindLogin(ctx context.Context, login string) (*core.User, error) {
	out := &core.User{Login: login}
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]interface{}{"user_login": login}
		query, args, err := binder.BindNamed(queryFindLogin, params)
		if err != nil {
			return err
		}
		row := queryer.QueryRow(query, args...)
		return scanRow(s.enc, row, out)
	})
	return out, err
}

// FindToken returns a user from the datastore by its token.
func (s *store) FindToken(ctx context.Context, token string) (*core.User, error) {
	out := &core.User{Token: token}
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		params := map[string]any{"user_token": token}
		query, args, err := binder.BindNamed(queryFindToken, params)
		if err != nil {
			return err
		}
		row := queryer.QueryRow(query, args...)
		return scanRow(s.enc, row, out)
	})
	return out, err
}

// List returns a list of users from the datastore.
func (s *store) List(ctx context.Context, q core.UserParams) ([]*core.User, error) {
	var out []*core.User
	err := s.db.View(func(queryer db.Queryer, binder db.Binder) error {
		limit := q.Limit + 1

		query := queryList
		params := map[string]any{"limit": limit}
		if q.After != "" {
			query = queryListOffset
			params["login"] = q.After
		}

		stmt, args, err := binder.BindNamed(query, params)
		if err != nil {
			return err
		}

		rows, err := queryer.Query(stmt, args...)
		if err != nil {
			return err
		}

		defer func() {
			if err := rows.Close(); err != nil {
				logger.FromContext(ctx).
					WithError(err).
					Warnln("store: cannot close user rows")
			}
		}()

		out, err = scanRows(s.enc, rows)
		return err
	})
	return out, err
}

// Update persists an update user to the datastore.
func (s *store) Update(ctx context.Context, user *core.User) error {
	return s.db.Lock(func(execer db.Execer, binder db.Binder) error {
		params, err := toParams(s.enc, user)
		if err != nil {
			return err
		}
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
	user_id,
	user_login,
	user_email,
	user_admin,
	user_active,
	user_avatar,
	user_created,
	user_updated,
	user_synced,
	user_syncing,
	user_oauth_token,
	user_oauth_refresh,
	user_oauth_expiry,
	user_token
`

const queryCount = `
SELECT COUNT(*) FROM users
`

const queryFind = queryBase + `
FROM users
WHERE user_id = :user_id
`

const queryFindLogin = queryBase + `
FROM users
WHERE user_login = :user_login
`

const queryFindToken = queryBase + `
FROM users
WHERE user_token = :user_token
`

const queryList = queryBase + `
FROM users
ORDER BY user_login
LIMIT :limit
`

const queryListOffset = queryBase + `
FROM users
WHERE user_login > :login
ORDER BY user_login
LIMIT :limit
`

const stmtDelete = `
DELETE FROM users WHERE user_id = :user_id
`

const stmtInsert = `
INSERT INTO users (
	user_login,
	user_email,
	user_admin,
	user_active,
	user_avatar,
	user_created,
	user_updated,
	user_synced,
	user_syncing,
	user_oauth_token,
	user_oauth_refresh,
	user_oauth_expiry,
	user_token
) VALUES (
	:user_login,
	:user_email,
	:user_admin,
	:user_active,
	:user_avatar,
	:user_created,
	:user_updated,
	:user_synced,
	:user_syncing,
	:user_oauth_token,
	:user_oauth_refresh,
	:user_oauth_expiry,
	:user_token
)
`

const stmtInsertPg = stmtInsert + `
RETURNING user_id
`

const stmtUpdate = `
UPDATE users
SET
	user_email         = :user_email,
	user_admin         = :user_admin,
	user_active        = :user_active,
	user_avatar        = :user_avatar,
	user_created       = :user_created,
	user_updated       = :user_updated,
	user_synced        = :user_synced,
	user_syncing       = :user_syncing,
	user_oauth_token   = :user_oauth_token,
	user_oauth_refresh = :user_oauth_refresh,
	user_oauth_expiry  = :user_oauth_expiry,
	user_token         = :user_token
WHERE user_id = :user_id
`
