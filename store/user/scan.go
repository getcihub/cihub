package user

import (
	"database/sql"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
	"github.com/getcihub/cihub/store/shared/encrypter"
)

// helper function converts the User structure to a set
// of named query parameters.
func toParams(encrypt encrypter.Encrypter, u *core.User) (map[string]interface{}, error) {
	token, err := encrypt.Encrypt(u.Access)
	if err != nil {
		return nil, err
	}
	refresh, err := encrypt.Encrypt(u.Refresh)
	if err != nil {
		return nil, err
	}
	return map[string]interface{}{
		"user_id":            u.ID,
		"user_login":         u.Login,
		"user_email":         u.Email,
		"user_admin":         u.Admin,
		"user_active":        u.Active,
		"user_avatar":        u.Avatar,
		"user_created":       u.Created,
		"user_updated":       u.Updated,
		"user_synced":        u.Synced,
		"user_syncing":       u.Syncing,
		"user_oauth_token":   token,
		"user_oauth_refresh": refresh,
		"user_oauth_expiry":  u.Expiry,
		"user_token":         u.Token,
	}, nil
}

// helper function scans the sql.Row and copies the column
// values to the destination object.
func scanRow(encrypt encrypter.Encrypter, scanner db.Scanner, dest *core.User) error {
	var access, refresh []byte
	err := scanner.Scan(
		&dest.ID,
		&dest.Login,
		&dest.Email,
		&dest.Admin,
		&dest.Active,
		&dest.Avatar,
		&dest.Created,
		&dest.Updated,
		&dest.Synced,
		&dest.Syncing,
		&access,
		&refresh,
		&dest.Expiry,
		&dest.Token,
	)
	if err != nil {
		return err
	}
	dest.Access, err = encrypt.Decrypt(access)
	if err != nil {
		return err
	}
	dest.Refresh, err = encrypt.Decrypt(refresh)
	if err != nil {
		return err
	}
	return nil
}

// helper function scans the sql.Row and copies the column
// values to the destination object.
func scanRows(encrypt encrypter.Encrypter, rows *sql.Rows) ([]*core.User, error) {
	users := []*core.User{}
	for rows.Next() {
		user := new(core.User)
		err := scanRow(encrypt, rows, user)
		if err != nil {
			return nil, err
		}
		users = append(users, user)
	}

	return users, nil
}
