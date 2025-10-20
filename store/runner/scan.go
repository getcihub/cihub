package runner

import (
	"database/sql"

	"github.com/getcihub/cihub/core"
	"github.com/getcihub/cihub/store/shared/db"
	"github.com/getcihub/cihub/store/shared/encrypter"
)

// helper function converts the Runner structure to a set
// of named query parameters.
func toParams(encrypt encrypter.Encrypter, r *core.Runner) (map[string]interface{}, error) {
	token, err := encrypt.Encrypt(r.Token)
	if err != nil {
		return nil, err
	}

	return map[string]interface{}{
		"runner_name":            r.Name,
		"runner_id":              r.ID,
		"runner_installation_id": r.InstallationID,
		"runner_owner":           r.Owner,
		"runner_repo":            r.Repo,
		"runner_status":          r.Status,
		"runner_assigned_to":     r.AssignedTo,
		"runner_cancelled":       r.Cancelled,
		"runner_completed":       r.Completed,
		"runner_created":         r.Created,
		"runner_started":         r.Started,
		"runner_stopped":         r.Stopped,
		"runner_updated":         r.Updated,
		"runner_timeout":         r.Timeout,
		"runner_token":           token,
	}, nil
}

// helper function scans the sql.Row and copies the column
// values to the destination object.
func scanRow(encrypt encrypter.Encrypter, scanner db.Scanner, dest *core.Runner) error {
	var token []byte

	err := scanner.Scan(
		&dest.Name,
		&dest.ID,
		&dest.InstallationID,
		&dest.Owner,
		&dest.Repo,
		&dest.Status,
		&dest.AssignedTo,
		&dest.Cancelled,
		&dest.Completed,
		&dest.Created,
		&dest.Started,
		&dest.Stopped,
		&dest.Updated,
		&dest.Timeout,
		&token,
	)
	if err != nil {
		return err
	}

	dest.Token, err = encrypt.Decrypt(token)
	if err != nil {
		return err
	}

	return nil
}

// helper function scans the sql.Rows and copies the column
// values to the destination object.
func scanRows(encrypt encrypter.Encrypter, rows *sql.Rows) ([]*core.Runner, error) {
	runners := []*core.Runner{}
	for rows.Next() {
		runner := new(core.Runner)
		err := scanRow(encrypt, rows, runner)
		if err != nil {
			return nil, err
		}
		runners = append(runners, runner)
	}

	return runners, nil
}
