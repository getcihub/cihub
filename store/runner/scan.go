package runner

import (
	"database/sql"
	"strings"

	"github.com/sirupsen/logrus"

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

	var labels string
	if len(r.Labels) > 0 {
		labels = strings.Join(r.Labels, ",")
	}

	return map[string]interface{}{
		"runner_name":            r.Name,
		"runner_id":              r.ID,
		"runner_installation_id": r.InstallationID,
		"runner_owner":           r.Owner,
		"runner_status":          r.Status,
		"runner_machine":         r.Machine,
		"runner_arch":            r.Arch,
		"runner_cpu":             r.CPU,
		"runner_ram":             r.RAM,
		"runner_image":           r.Image,
		"runner_group_id":        r.GroupID,
		"runner_labels":          labels,
		"runner_cancelled":       r.Cancelled,
		"runner_created":         r.Created,
		"runner_accepted":        r.Accepted,
		"runner_started":         r.Started,
		"runner_stopped":         r.Stopped,
		"runner_updated":         r.Updated,
		"runner_token":           token,
	}, nil
}

// helper function scans the sql.Row and copies the column
// values to the destination object.
func scanRow(encrypt encrypter.Encrypter, scanner db.Scanner, dest *core.Runner) error {
	var token []byte
	var labels string

	err := scanner.Scan(
		&dest.Name,
		&dest.ID,
		&dest.InstallationID,
		&dest.Owner,
		&dest.Status,
		&dest.Machine,
		&dest.Arch,
		&dest.CPU,
		&dest.RAM,
		&dest.Image,
		&dest.GroupID,
		&labels,
		&dest.Cancelled,
		&dest.Created,
		&dest.Accepted,
		&dest.Started,
		&dest.Stopped,
		&dest.Updated,
		&token,
	)
	if err != nil {
		return err
	}

	if labels != "" {
		dest.Labels = strings.Split(labels, ",")
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
	defer func() {
		if err := rows.Close(); err != nil {
			logrus.WithError(err).
				Warnln("store: cannot close runner rows")
		}
	}()

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
